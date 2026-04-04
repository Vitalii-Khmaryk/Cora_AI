//! # CORA — Local AI Gaming Assistant (Tauri 2.0 Backend)
//!
//! All heavy lifting (STT, LLM, TTS, hotkey, wake word) runs in
//! background threads/tasks. The Tauri window (React frontend) receives
//! state updates via `app_handle.emit("cora-state", ...)`.

mod audio;
mod config;
mod constants;
mod hotkey;
mod llm;
mod stt;
mod state;
mod tts;
mod vision;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use log::{debug, error, info, warn};
use tauri::Emitter;
use tokio::sync::mpsc;

use crate::constants::*;
use crate::state::{ActivationSource, Status};

// ═══════════════════════════════════════════════════════════════════
// Tauri Entry Point
// ═══════════════════════════════════════════════════════════════════

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables early
    dotenvy::dotenv().ok();

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("CORA — AI Gaming Assistant v6.0.0 (Tauri)");

    // ── Shared state created before Builder so both setup() and
    //    on_window_event() can capture clones ──
    let whisper_child: Arc<std::sync::Mutex<Option<std::process::Child>>> =
        Arc::new(std::sync::Mutex::new(None));
    let should_quit = Arc::new(AtomicBool::new(false));

    // Clones for the on_window_event closure
    let wc_cleanup = whisper_child.clone();
    let sq_cleanup = should_quit.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::save_config
        ])
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                sq_cleanup.store(true, Ordering::SeqCst);

                if let Ok(mut guard) = wc_cleanup.lock() {
                    if let Some(ref mut child) = *guard {
                        info!("Shutting down Whisper server (PID {})...", child.id());
                        match child.kill() {
                            Ok(()) => {
                                let _ = child.wait();
                                info!("Whisper server stopped.");
                            }
                            Err(e) => warn!("Failed to kill Whisper server: {e}"),
                        }
                    }
                }
            }
        })
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // ── Auto-start Whisper server ──
            {
                let config = crate::config::get_config(app_handle.clone());
                let child = match stt::start_whisper_server(&config.language) {
                    Ok(child) => {
                        info!(
                            "Whisper server started (PID {}) on port {WHISPER_PORT}.",
                            child.id()
                        );
                        Some(child)
                    }
                    Err(e) => {
                        warn!("Could not auto-start Whisper server: {e:#}");
                        warn!("Assuming it is already running or will be started manually.");
                        None
                    }
                };
                *whisper_child.lock().unwrap() = child;
            }

            // Shared channel: hotkey/vosk threads → main async loop
            let (tx, rx) = mpsc::channel::<ActivationSource>(16);

            // Shared flag: prevents double-activation and pauses vosk during recording
            let is_recording = Arc::new(AtomicBool::new(false));

            // Shared state for UI overlay (future React integration)
            let (status_tx, _status_rx) = tokio::sync::watch::channel(Status::Idle);
            let audio_amplitude = Arc::new(AtomicU32::new(0));

            // ── Spawn Global Hotkey Listener Thread ──
            {
                let tx = tx.clone();
                let flag = is_recording.clone();
                std::thread::Builder::new()
                    .name("hotkey-listener".into())
                    .spawn(move || {
                        if let Err(e) = hotkey::run_hotkey_listener(tx, flag) {
                            error!("Hotkey listener failed: {e:#}");
                        }
                    })
                    .expect("Failed to spawn hotkey thread");
            }

            // ── Spawn Vosk Wake Word Listener Thread ──
            {
                let tx = tx.clone();
                let flag = is_recording.clone();
                std::thread::Builder::new()
                    .name("vosk-listener".into())
                    .spawn(move || {
                        if let Err(e) = stt::run_vosk_listener(tx, flag) {
                            error!("Vosk listener failed: {e:#}");
                            warn!("Wake word detection is disabled. Use Ctrl+Shift+C instead.");
                        }
                    })
                    .expect("Failed to spawn vosk thread");
            }

            // Drop the original sender so the channel closes when all threads exit
            drop(tx);

            // ── Spawn the backend Tokio task (main pipeline loop) ──
            {
                let is_recording = is_recording.clone();
                let should_quit = should_quit.clone();
                let audio_amplitude = audio_amplitude.clone();

                tauri::async_runtime::spawn(async move {
                    if let Err(e) = async_main(
                        rx,
                        is_recording,
                        should_quit,
                        status_tx,
                        audio_amplitude,
                        app_handle,
                    )
                    .await
                    {
                        error!("Async main loop failed: {e:#}");
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ═══════════════════════════════════════════════════════════════════
// Async Main Loop
// ═══════════════════════════════════════════════════════════════════

async fn async_main(
    mut rx: mpsc::Receiver<ActivationSource>,
    is_recording: Arc<AtomicBool>,
    should_quit: Arc<AtomicBool>,
    status_tx: tokio::sync::watch::Sender<Status>,
    audio_amplitude: Arc<AtomicU32>,
    app_handle: tauri::AppHandle,
) -> anyhow::Result<()> {
    info!("Cora is ready and listening.");
    info!("  → Press Ctrl+Shift+C to activate");
    info!("  → Or say 'Cora' to activate (requires Vosk model)");

    // Reuse a single reqwest client for connection pooling
    let http = reqwest::Client::new();

    // Persistent session memory (ephemeral, in-memory only)
    let mut history = llm::SessionHistory::new();

    // Helper to emit state events to the frontend
    let emit = |state: &str| {
        let _ = app_handle.emit("cora-state", state);
    };

    emit("idle");

    loop {
        // Check shutdown between iterations
        if should_quit.load(Ordering::SeqCst) {
            info!("Quit requested. Shutting down.");
            break;
        }

        // Non-blocking recv with timeout so we periodically check should_quit
        let source = tokio::select! {
            maybe = rx.recv() => {
                match maybe {
                    Some(s) => s,
                    None => break, // all senders dropped
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => continue,
        };

        // Guard against double-activations
        if is_recording.load(Ordering::SeqCst) {
            debug!("Already recording, ignoring duplicate activation.");
            continue;
        }
        is_recording.store(true, Ordering::SeqCst);
        let _ = status_tx.send(Status::Listening);
        emit("listening");

        info!("━━━ Activated via {source:?} ━━━");

        // Activation beep (high pitch)
        audio::play_beep(880.0, 100);

        // Speculative vision capture — runs in parallel with recording
        let screenshot_handle = tokio::spawn(async {
            match vision::capture_screen_base64().await {
                Ok(img) => {
                    info!(
                        "Speculative screenshot captured ({} KB base64)",
                        img.len() / 1024
                    );
                    Some(img)
                }
                Err(e) => {
                    warn!("Speculative screenshot failed (non-fatal): {e:#}");
                    None
                }
            }
        });

        // Record audio → in-memory WAV (no disk I/O)
        emit("recording");
        let _ = status_tx.send(Status::Recording);

        let wav_bytes = match audio::record_with_vad(audio_amplitude.clone()).await {
            Ok(bytes) => {
                // Recording-stopped beep (low pitch)
                audio::play_beep(440.0, 50);
                info!("Audio recorded: {} bytes in memory", bytes.len());
                bytes
            }
            Err(e) => {
                error!("Recording failed: {e:#}");
                let _ = status_tx.send(Status::Idle);
                emit("idle");
                is_recording.store(false, Ordering::SeqCst);
                continue;
            }
        };

        // Speech-to-Text via Whisper (in-memory)
        emit("transcribing");
        let _ = status_tx.send(Status::Transcribing);

        let transcription = match stt::transcribe_audio(&http, wav_bytes, app_handle.clone()).await {
            Ok(text) => text,
            Err(e) => {
                error!("Whisper STT failed: {e:#}");
                let _ = status_tx.send(Status::Idle);
                emit("idle");
                is_recording.store(false, Ordering::SeqCst);
                continue;
            }
        };

        // Skip empty / noise-only transcriptions
        let trimmed = transcription.trim();
        if trimmed.is_empty()
            || trimmed == "."
            || trimmed == "[BLANK_AUDIO]"
            || trimmed.starts_with('[')
        {
            info!("Transcription empty or noise — skipping.");
            let _ = status_tx.send(Status::Idle);
            emit("idle");
            is_recording.store(false, Ordering::SeqCst);
            continue;
        }
        info!("Transcription: \"{trimmed}\"");

        // LLM Inference
        emit("thinking");
        let _ = status_tx.send(Status::Thinking);

        // Await speculative screenshot
        let screenshot = screenshot_handle.await.unwrap_or(None);

        // Create TTS sentence channel + worker
        let (tx_tts, rx_tts) = mpsc::channel::<String>(32);
        let tts_handle = tokio::spawn(tts::run_tts_worker(rx_tts, audio_amplitude.clone(), app_handle.clone()));

        // Stream LLM tokens → sentence-level TTS
        let full_response = match llm::ask_ollama_streaming(
            &http,
            &mut history,
            trimmed,
            screenshot,
            tx_tts,
            status_tx.clone(),
        )
        .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("Ollama streaming failed: {e:#}");
                let _ = status_tx.send(Status::Idle);
                emit("idle");
                is_recording.store(false, Ordering::SeqCst);
                continue;
            }
        };

        // Note: when LLM starts producing tokens, llm.rs sends Status::Speaking
        // and the emit("speaking") will be handled there through the status_tx.
        // We emit it here as well for the frontend event hook.
        emit("speaking");

        info!("CORA: {}", full_response.replace('\n', " "));

        // Wait for TTS worker to finish speaking all queued sentences
        let _ = tts_handle.await;

        let _ = status_tx.send(Status::Idle);
        emit("idle");
        is_recording.store(false, Ordering::SeqCst);
        info!("Ready for next activation.\n");
    }

    Ok(())
}
