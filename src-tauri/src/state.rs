//! Shared state types used across Cora's backend modules.

use serde::Serialize;

/// How Cora was activated.
#[derive(Debug)]
pub enum ActivationSource {
    Hotkey,
    WakeWord,
}

/// Current system status — drives UI updates via Tauri events.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Idle,
    Listening,
    Recording,
    Transcribing,
    Thinking,
    Speaking,
}
