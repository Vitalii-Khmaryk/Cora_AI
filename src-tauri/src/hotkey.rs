//! Global hotkey listener — Ctrl+Shift+C activation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use log::info;
use tokio::sync::mpsc;

use crate::state::ActivationSource;

/// Runs on a dedicated OS thread. Blocks on the crossbeam receiver
/// waiting for hotkey events, then forwards them to the async channel.
pub fn run_hotkey_listener(
    tx: mpsc::Sender<ActivationSource>,
    is_recording: Arc<AtomicBool>,
) -> Result<()> {
    let manager = GlobalHotKeyManager::new()
        .map_err(|e| anyhow::anyhow!("Failed to create hotkey manager: {e}"))?;

    let hotkey = HotKey::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyC,
    );
    manager
        .register(hotkey)
        .map_err(|e| anyhow::anyhow!("Failed to register Ctrl+Shift+C: {e}"))?;

    info!("Global hotkey registered: Ctrl+Shift+C");

    // GlobalHotKeyEvent::receiver() returns a crossbeam channel receiver.
    // On Windows, the crate internally manages the message pump thread.
    let receiver = GlobalHotKeyEvent::receiver();

    loop {
        // This blocks until a hotkey event is fired
        if let Ok(_event) = receiver.recv() {
            if !is_recording.load(Ordering::SeqCst) {
                info!("Hotkey Ctrl+Shift+C pressed.");
                let _ = tx.blocking_send(ActivationSource::Hotkey);
            }
        }
    }
}
