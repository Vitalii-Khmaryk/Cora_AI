//! Piper neural TTS — zero disk I/O, fully in-memory pipeline.
//!
//! Piper writes raw PCM data to stdout (`--output_raw`), we capture
//! the bytes in memory, and play them directly via rodio.
//! No temporary audio files are ever created.
//!
//! The `run_tts_worker()` task consumes sentences from an mpsc channel and
//! speaks them sequentially, enabling the LLM to stream tokens while the
//! first sentences are already being spoken.
use std::path::PathBuf;

use anyhow::{Context, Result};
use log::{debug, info, warn};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};



// ═══════════════════════════════════════════════════════════════════
// TTS Worker (sentence-level streaming consumer)
// ═══════════════════════════════════════════════════════════════════

/// TTS worker task — consumes sentences from the channel and speaks
/// them sequentially via Piper. Exits when the sender is dropped
/// (i.e., when the LLM stream finishes and the channel closes).
pub async fn run_tts_worker(mut rx: mpsc::Receiver<String>, audio_amplitude: Arc<AtomicU32>, app_handle: tauri::AppHandle) {
    debug!("TTS worker started — waiting for sentences.");

    while let Some(sentence) = rx.recv().await {
        speak_sentence(sentence, audio_amplitude.clone(), app_handle.clone()).await;
    }

    debug!("TTS worker finished — channel closed.");
}

// ═══════════════════════════════════════════════════════════════════
// Piper Neural TTS — In-Memory Pipeline
// ═══════════════════════════════════════════════════════════════════

/// Speaks a single sentence using Piper neural TTS.
///
/// Pipeline: text → piper.exe (stdin) → stdout raw bytes → rodio playback.
/// **Zero disk I/O** — everything stays in memory.
pub async fn speak_sentence(text: String, audio_amplitude: Arc<AtomicU32>, app_handle: tauri::AppHandle) {
    // Sanitize text for TTS (remove control chars, markdown, quotes)
    let filtered = text
        .replace(['"', '*'], "")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>();

    if filtered.trim().is_empty() {
        debug!("TTS text empty after filtering — skipping.");
        return;
    }

    // Step 1: Run piper.exe → capture WAV bytes from stdout
    let wav_bytes = match run_piper_to_memory(&filtered, app_handle).await {
        Ok(bytes) => {
            debug!("Piper generated {} bytes in memory", bytes.len());
            bytes
        }
        Err(e) => {
            warn!("Piper TTS failed (non-fatal): {e:#}");
            return;
        }
    };

    if wav_bytes.is_empty() {
        warn!("Piper produced empty output — skipping playback.");
        return;
    }

    // Step 2: Play the raw PCM bytes directly from memory via rodio
    let handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = play_wav_from_memory(&wav_bytes, audio_amplitude) {
            warn!("WAV playback failed (non-fatal): {e:#}");
        }
    });
    let _ = handle.await;
}

/// Runs `piper.exe` as a child process, passing text via stdin.
/// Piper writes the synthesized raw audio to stdout (`--output_raw`).
/// Returns the complete audio data as a byte vector.
async fn run_piper_to_memory(text: &str, app_handle: tauri::AppHandle) -> Result<Vec<u8>> {
    use tokio::io::AsyncWriteExt;

    let piper_path = PathBuf::from("models/piper/piper.exe");
    if !piper_path.exists() {
        anyhow::bail!(
            "Piper executable not found at '{}'. \
             See setup instructions.",
            piper_path.display()
        );
    }

    // Dynamically load the language from settings
    let config = crate::config::get_config(app_handle);
    let model_path = PathBuf::from(format!("models/piper/{}.onnx", config.language));

    if !model_path.exists() {
        anyhow::bail!(
            "Piper voice model not found at '{}'. \
             Ensure it is downloaded.", model_path.display()
        );
    }

    let mut child = tokio::process::Command::new(&piper_path)
        .arg("--model")
        .arg(&model_path)
        .arg("--output_raw")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped()) // Capture stdout
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn piper.exe")?;

    // Write text to piper's stdin, then drop to signal EOF
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).await?;
        drop(stdin);
    }

    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for piper.exe")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Piper exited with {}: {stderr}", output.status);
    }

    Ok(output.stdout)
}

// ═══════════════════════════════════════════════════════════════════
// In-Memory Raw PCM Playback (via Rodio)
// ═══════════════════════════════════════════════════════════════════
fn play_wav_from_memory(wav_data: &[u8], audio_amplitude: Arc<AtomicU32>) -> Result<()> {
    use rodio::{OutputStream, Sink, buffer::SamplesBuffer};

    if wav_data.is_empty() {
        warn!("Audio data is empty — nothing to play.");
        return Ok(());
    }

    info!("Playing TTS: {} raw bytes (in-memory)", wav_data.len());

    let mut i16_samples = Vec::with_capacity(wav_data.len() / 2);
    for chunk in wav_data.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        i16_samples.push(sample);
    }

    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to get default audio output")?;
    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    let sample_rate: u32 = 22050; // Piper output default
    let samples_per_chunk = (sample_rate as usize * 50) / 1000; // ~50ms

    for chunk in i16_samples.chunks(samples_per_chunk) {
        // Calculate RMS of chunk
        let sum_sq: f32 = chunk.iter().map(|&s| {
            let f = s as f32 / i16::MAX as f32;
            f * f
        }).sum();
        
        let rms = if chunk.is_empty() { 
            0.0 
        } else { 
            (sum_sq / chunk.len() as f32).sqrt() 
        };
        
        audio_amplitude.store(rms.to_bits(), Ordering::Relaxed);
        
        // Append small chunks to keep playback continuous
        let source = SamplesBuffer::new(1, sample_rate, chunk.to_vec());
        sink.append(source);
        
        // Back-pressure logic: wait until the sink has processed enough chunks
        while sink.len() > 2 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    sink.sleep_until_end();
    audio_amplitude.store(0.0f32.to_bits(), Ordering::Relaxed);

    Ok(())
}
