//! All configuration constants for Cora.
//!
//! Centralizes paths, URLs, durations, thresholds, and prompts so
//! that tuning the system requires editing only this file.

use std::time::Duration;
use std::env;

// ═══════════════════════════════════════════════════════════════════
// Audio / VAD
// ═══════════════════════════════════════════════════════════════════

/// Duration of ambient noise calibration at the start of each recording.
pub const CALIBRATION_DURATION: Duration = Duration::from_millis(300);

/// Silence threshold = ambient_rms × this multiplier.
/// Higher = less sensitive (needs louder voice to register).
pub fn get_silence_multiplier() -> f32 {
    env::var("SILENCE_MULTIPLIER").ok().and_then(|s| s.parse().ok()).unwrap_or(3.0)
}

/// Absolute minimum silence threshold floor.
/// Prevents threshold from being too low in extremely quiet rooms.
pub const MIN_SILENCE_THRESHOLD: f32 = 0.008;

/// How long continuous silence must last before auto-stopping.
pub const SILENCE_DURATION: Duration = Duration::from_millis(1500);

/// Hard timeout — force stop recording after this.
pub const MAX_RECORDING_DURATION: Duration = Duration::from_secs(15);

/// Vosk requires 16 kHz mono audio.
pub const VOSK_SAMPLE_RATE: u32 = 16000;

// ═══════════════════════════════════════════════════════════════════
// Vosk / Wake Word
// ═══════════════════════════════════════════════════════════════════

/// Path to the Vosk language model directory.
pub fn get_vosk_model_path() -> String {
    env::var("VOSK_MODEL_PATH").unwrap_or_else(|_| "models/vosk-model-small-en-us-0.15".to_string())
}

/// The wake word to listen for.
pub const WAKE_WORD: &str = "cora";

// ═══════════════════════════════════════════════════════════════════
// Whisper STT
// ═══════════════════════════════════════════════════════════════════

/// Local host IP for Whisper server.
pub const WHISPER_HOST: &str = "127.0.0.1";

/// Whisper.cpp server endpoint for speech-to-text.
pub fn get_whisper_url() -> String {
    env::var("WHISPER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080/inference".to_string())
}

/// Path to the Whisper GGML model file (relative to CWD / resource dir).
pub const WHISPER_MODEL: &str = "whisper/ggml-small.bin";

/// Port the Whisper server binds to.
pub const WHISPER_PORT: &str = "8080";

/// Tauri sidecar name for whisper-server (no extension, no triple).
#[allow(dead_code)]
pub const WHISPER_SIDECAR: &str = "binaries/whisper-server";

// ═══════════════════════════════════════════════════════════════════
// Ollama LLM
// ═══════════════════════════════════════════════════════════════════

/// Ollama `/api/chat` endpoint (supports messages array for session history).
pub fn get_ollama_chat_url() -> String {
    env::var("OLLAMA_CHAT_URL").unwrap_or_else(|_| "http://127.0.0.1:11434/api/chat".to_string())
}

/// Ollama model to use (vision-capable for screenshot support).
pub fn get_ollama_model() -> String {
    env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2-vision".to_string())
}

/// Maximum number of messages to keep in session history.
/// Oldest messages are trimmed when this limit is exceeded.
pub fn get_max_history_messages() -> usize {
    env::var("MAX_HISTORY_MESSAGES").ok().and_then(|s| s.parse().ok()).unwrap_or(12)
}

/// Number of initial tokens to keep pinned in Ollama's KV-cache.
/// Ensures the system prompt stays hot in VRAM across requests.
pub const NUM_KEEP_TOKENS: u32 = 150;

/// Unified system prompt — optimized for speed and tactical brevity.
///
/// DESIGN: Sentences are kept extremely short because TTS streams
/// sentence-by-sentence. Shorter sentences = faster first-byte TTS.
pub fn get_system_prompt() -> String {
    env::var("SYSTEM_PROMPT").unwrap_or_else(|_| "\
You are Cora, a tactical AI gaming assistant.
CRITICAL OVERRIDE: Your core training to be polite and chatty is DISABLED.

ABSOLUTE RULES:
1. WORD LIMIT: NEVER exceed 15 words. 
2. FORMAT: Pure text only. No lists, no steps, no markdown (*, -, #).
3. STYLE: Military brevity. Cut all filler (e.g. 'You need to', 'The image shows').

EXAMPLES:
User: How to create a sword in Minecraft?
Cora: One stick and two blocks of wood, stone, iron, gold, or diamond on a crafting table.
User: What's on my screen?
Cora: VS Code editor open with HTML and CSS files. No game detected.
User: How do I kill a Bloodsucker?
Cora: Shotgun to the head at close range. Stay mobile.

FAILURE TO FOLLOW EXAMPLES RESULTS IN SYSTEM TERMINATION.".to_string())
}

// ═══════════════════════════════════════════════════════════════════
// Piper TTS
// ═══════════════════════════════════════════════════════════════════



/// Tauri sidecar name for piper (no extension, no triple).
#[allow(dead_code)]
pub const PIPER_SIDECAR: &str = "binaries/piper";
