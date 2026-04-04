//! Speech-to-text: Vosk wake word listener, Whisper transcription,
//! and Whisper server auto-start (Tauri sidecar).
//!
//! Whisper STT accepts in-memory WAV bytes (no disk I/O).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{debug, error, info, warn};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::state::ActivationSource;
use crate::constants::*;

// ═══════════════════════════════════════════════════════════════════
// Vosk Wake Word Listener (runs on dedicated OS thread)
// ═══════════════════════════════════════════════════════════════════

pub fn run_vosk_listener(
    tx: mpsc::Sender<ActivationSource>,
    is_recording: Arc<AtomicBool>,
) -> Result<()> {
    let vosk_path = crate::constants::get_vosk_model_path();
    info!("Loading Vosk model from '{vosk_path}'...");

    let model = vosk::Model::new(&vosk_path)
        .context("Failed to load Vosk model. See setup instructions in module docs.")?;

    // Grammar-restricted recognizer: only match "cora" or unknown speech.
    // This is far more CPU-efficient than free-form recognition.
    let mut recognizer =
        vosk::Recognizer::new_with_grammar(&model, VOSK_SAMPLE_RATE as f32, &["cora", "[unk]"])
            .context("Failed to create Vosk recognizer")?;

    // Open a dedicated cpal input stream for wake word detection
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No audio input device found for Vosk")?;
    let supported = device.default_input_config()?;
    let native_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;

    info!("Vosk input: {native_rate} Hz, {channels} channel(s)");

    // std::sync::mpsc is used here because the cpal callback runs on an OS thread,
    // not inside the tokio runtime. This avoids any async overhead in the hot path.
    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();

    let stream_config: cpal::StreamConfig = supported.into();
    let stream = device.build_input_stream(
        &stream_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Non-blocking send — if the receiver is behind, we drop the chunk.
            // This is acceptable for wake word detection.
            let _ = audio_tx.send(data.to_vec());
        },
        |e| error!("Vosk audio stream error: {e}"),
        None,
    )?;
    stream.play()?;

    info!("Wake word listener active — say 'Cora' to activate.");

    let mut mono_buf = Vec::new();
    let mut resampled_buf = Vec::new();
    let mut pcm_i16_buf = Vec::new();

    loop {
        let samples = match audio_rx.recv() {
            Ok(s) => s,
            Err(_) => {
                warn!("Vosk audio channel closed.");
                break;
            }
        };

        // Skip processing while the main loop is recording
        if is_recording.load(Ordering::Relaxed) {
            continue;
        }

        // ── Mono mixdown ──
        mono_buf.clear();
        if channels > 1 {
            mono_buf.extend(
                samples
                    .chunks(channels)
                    .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            );
        } else {
            mono_buf.extend_from_slice(&samples);
        }

        // ── Downsample to 16 kHz (nearest-neighbor) ──
        resampled_buf.clear();
        if native_rate == VOSK_SAMPLE_RATE {
            resampled_buf.extend_from_slice(&mono_buf);
        } else {
            let ratio = native_rate as f64 / VOSK_SAMPLE_RATE as f64;
            let output_len = (mono_buf.len() as f64 / ratio) as usize;
            resampled_buf.extend((0..output_len).map(|i| {
                let src_idx = (i as f64 * ratio) as usize;
                mono_buf[src_idx.min(mono_buf.len().saturating_sub(1))]
            }));
        }

        // ── Convert f32 [-1.0, 1.0] → i16 for Vosk ──
        pcm_i16_buf.clear();
        pcm_i16_buf.extend(
            resampled_buf
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        );

        // Feed audio to the recognizer
        recognizer.accept_waveform(&pcm_i16_buf);

        // Check partial results for the wake word
        let partial = recognizer.partial_result().partial.to_lowercase();
        if partial.contains(WAKE_WORD) {
            info!("Wake word 'Cora' detected!");
            let _ = tx.blocking_send(ActivationSource::WakeWord);
            recognizer.reset();

            // Cooldown to prevent rapid re-triggers
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Whisper Speech-to-Text (in-memory)
// ═══════════════════════════════════════════════════════════════════

/// Whisper.cpp JSON response schema.
#[derive(Debug, Deserialize)]
struct WhisperResponse {
    /// The transcribed text.
    text: String,
}

/// Sends an in-memory WAV buffer to the local whisper.cpp server as
/// multipart/form-data and returns the transcribed English text.
///
/// **Zero disk I/O** — the WAV bytes never touch the filesystem.
pub async fn transcribe_audio(http: &reqwest::Client, wav_bytes: Vec<u8>, app_handle: tauri::AppHandle) -> Result<String> {
    let whisper_url = crate::constants::get_whisper_url();
    info!(
        "Sending {} bytes to Whisper at {whisper_url}...",
        wav_bytes.len()
    );

    let config = crate::config::get_config(app_handle);
    let lang = config.language;

    let file_part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("command.wav")
        .mime_str("audio/wav")?;

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("language", lang);

    let response = http
        .post(&whisper_url)
        .multipart(form)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .context("Failed to connect to Whisper server — is it running on port 8080?")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Whisper returned HTTP {status}: {body}");
    }

    let body = response.text().await?;
    debug!("Whisper raw response: {body}");

    // whisper.cpp returns {"text": "..."}  — parse it
    let parsed: WhisperResponse =
        serde_json::from_str(&body).context("Failed to parse Whisper JSON response")?;

    Ok(parsed.text.trim().to_string())
}

// ═══════════════════════════════════════════════════════════════════
// Whisper Server Auto-Start (Tauri Sidecar)
// ═══════════════════════════════════════════════════════════════════

/// Spawns `whisper-server.exe` as a hidden background process.
///
/// Uses `CREATE_NO_WINDOW` (0x08000000) on Windows so no console window
/// pops up. Waits for the server to bind to the port before returning.
pub fn start_whisper_server(lang: &str) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt;

    // In Tauri dev mode, CWD is src-tauri/, so relative paths work.
    // For sidecar, we use the exe directly from the whisper/ folder.
    let exe_path = PathBuf::from("whisper/whisper-server.exe");
    if !exe_path.exists() {
        anyhow::bail!(
            "Whisper server not found at '{}'. \
             Place whisper-server.exe in the whisper/ folder.",
            exe_path.display()
        );
    }

    let model_path = PathBuf::from(WHISPER_MODEL);
    if !model_path.exists() {
        anyhow::bail!(
            "Whisper model not found at '{WHISPER_MODEL}'. \
             Download ggml-base.en.bin from https://huggingface.co/ggerganov/whisper.cpp"
        );
    }

    info!(
        "Starting Whisper server: {} -m {WHISPER_MODEL} --port {WHISPER_PORT} --language {lang}",
        exe_path.display()
    );

    /// Windows: CREATE_NO_WINDOW prevents a console window from appearing.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let child = std::process::Command::new(&exe_path)
        .arg("-m")
        .arg(&model_path)
        .arg("--port")
        .arg(WHISPER_PORT)
        .arg("--language")
        .arg(lang)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("Failed to spawn whisper-server.exe")?;

    info!(
        "Waiting for Whisper server to initialize on port {}...",
        WHISPER_PORT
    );

    let addr: std::net::SocketAddr = format!("{WHISPER_HOST}:{WHISPER_PORT}")
        .parse()
        .context("Failed to parse Whisper address")?;

    let start_wait = std::time::Instant::now();
    let max_wait = std::time::Duration::from_secs(10);
    let check_interval = std::time::Duration::from_millis(200);

    loop {
        if std::net::TcpStream::connect_timeout(&addr, check_interval).is_ok() {
            info!("Whisper server is ready.");
            break;
        }

        if start_wait.elapsed() > max_wait {
            anyhow::bail!(
                "Whisper server failed to start within {} seconds.",
                max_wait.as_secs()
            );
        }

        std::thread::sleep(check_interval);
    }

    Ok(child)
}
