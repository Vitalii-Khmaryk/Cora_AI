//! Ollama LLM integration — streaming `/api/chat` with session memory
//! and VRAM KV-cache pinning.
//!
//! Switched from `/api/generate` to `/api/chat` for multi-turn
//! conversation support. Tokens are streamed and buffered into sentences,
//! which are dispatched to the TTS channel as soon as they're formed.

use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::constants::*;
use crate::state::Status;

// ═══════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════

/// A single message in the Ollama `/api/chat` conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    /// Base64-encoded images. Only attached to the *current* user message,
    /// never stored in session history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

/// Ollama `/api/chat` streaming response chunk.
#[derive(Debug, Deserialize)]
struct StreamChunk {
    message: StreamMessage,
    done: bool,
}

/// The `message` field inside a streaming chunk.
#[derive(Debug, Deserialize)]
struct StreamMessage {
    content: String,
}

// ═══════════════════════════════════════════════════════════════════
// Session History
// ═══════════════════════════════════════════════════════════════════

/// In-memory rolling conversation history.
///
/// Stores the last `MAX_HISTORY_MESSAGES` (via env) user+assistant exchanges.
/// Screenshots are **never** stored — only text. This keeps VRAM usage
/// constant and prevents the context from ballooning.
pub struct SessionHistory {
    messages: Vec<Message>,
}

impl SessionHistory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Appends a user message (text only, no images) to history.
    pub fn push_user(&mut self, content: &str) {
        self.messages.push(Message {
            role: "user".into(),
            content: content.into(),
            images: None,
        });
        self.trim();
    }

    /// Appends an assistant response to history.
    pub fn push_assistant(&mut self, content: &str) {
        self.messages.push(Message {
            role: "assistant".into(),
            content: content.into(),
            images: None,
        });
        self.trim();
    }

    /// Trims old messages to stay within the rolling window.
    fn trim(&mut self) {
        let max_history = crate::constants::get_max_history_messages();
        while self.messages.len() > max_history {
            self.messages.remove(0);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Streaming LLM API
// ═══════════════════════════════════════════════════════════════════

/// Sends a user prompt (with optional screenshot) to Ollama via `/api/chat`
/// with token streaming enabled.
///
/// **Sentence-level TTS pipeline:** As tokens arrive, they are buffered.
/// When a sentence boundary (`.`, `!`, `?`) is detected, the complete
/// sentence is immediately dispatched to `tx_tts` so TTS can start
/// speaking while the LLM is still generating.
///
/// **Session memory:** After streaming completes, the user message (text
/// only, no images) and the full assistant response are appended to
/// `history`. Oldest messages are trimmed to the configured max history messages.
///
/// **VRAM caching:** The `options.num_keep` field pins the system prompt
/// tokens in Ollama's KV-cache across requests.
pub async fn ask_ollama_streaming(
    http: &reqwest::Client,
    history: &mut SessionHistory,
    user_text: &str,
    screenshot: Option<String>,
    tx_tts: mpsc::Sender<String>,
    status_tx: tokio::sync::watch::Sender<Status>,
) -> Result<String> {
    let ollama_model = crate::constants::get_ollama_model();
    info!("Streaming prompt to Ollama ({ollama_model}) via /api/chat...");
    debug!("Prompt: \"{user_text}\"");
    debug!(
        "History: {} messages, screenshot: {}",
        history.messages.len(),
        if screenshot.is_some() { "yes" } else { "no" }
    );

    // ── Build the messages array ──
    // [system] + [history...] + [current user message with optional image]
    let mut messages: Vec<Message> = Vec::with_capacity(history.messages.len() + 2);

    // System prompt — always first
    messages.push(Message {
        role: "system".into(),
        content: crate::constants::get_system_prompt(),
        images: None,
    });

    // Session history (text only, no images)
    messages.extend(history.messages.clone());

    // Current user message — attach screenshot if available
    messages.push(Message {
        role: "user".into(),
        content: user_text.into(),
        images: screenshot.map(|s| vec![s]),
    });

    // ── Build the request body ──
    let body = serde_json::json!({
        "model": ollama_model,
        "messages": messages,
        "stream": true,
        "options": {
            "num_keep": NUM_KEEP_TOKENS,
            "temperature": 0.1
        }
    });

    let response = http
        .post(crate::constants::get_ollama_chat_url())
        .json(&body)
        .timeout(Duration::from_secs(180))
        .send()
        .await
        .context("Failed to connect to Ollama — is it running on port 11434?")?;

    if !response.status().is_success() {
        let status = response.status();
        let err_body = response.text().await.unwrap_or_default();
        anyhow::bail!("Ollama returned HTTP {status}: {err_body}");
    }

    // ── Stream tokens and buffer sentences ──
    let mut stream = response.bytes_stream();
    let mut line_buf = String::new();
    let mut sentence_buf = String::new();
    let mut full_response = String::new();
    
    // Flag to indicate we've received the first token and started transitioning to 'Speaking'
    let mut started_speaking = false;

    while let Some(chunk_result) = stream.next().await {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                warn!("Ollama stream interrupted: {e:#}");
                break;
            }
        };

        // Accumulate bytes into a line buffer, process complete lines
        let text = String::from_utf8_lossy(&chunk);
        line_buf.push_str(&text);

        // Process all complete newline-delimited JSON lines
        while let Some(newline_pos) = line_buf.find('\n') {
            let line = line_buf[..newline_pos].trim().to_string();
            line_buf = line_buf[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let chunk_data: StreamChunk = match serde_json::from_str(&line) {
                Ok(c) => c,
                Err(e) => {
                    debug!("Skipping unparseable stream line: {e}");
                    continue;
                }
            };

            let token = &chunk_data.message.content;
            full_response.push_str(token);
            sentence_buf.push_str(token);

            if !started_speaking {
                started_speaking = true;
                let _ = status_tx.send(Status::Speaking);
            }

            // ── Sentence boundary detection ──
            // Flush the buffer when it ends with sentence-terminating punctuation.
            let trimmed = sentence_buf.trim();
            if !trimmed.is_empty()
                && (trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?'))
            {
                let sentence = trimmed.to_string();
                debug!("TTS sentence: \"{sentence}\"");
                let _ = tx_tts.send(sentence).await;
                sentence_buf.clear();
            }

            if chunk_data.done {
                break;
            }
        }
    }

    // ── Flush any remaining tokens that didn't end with punctuation ──
    let remaining = sentence_buf.trim().to_string();
    if !remaining.is_empty() {
        debug!("TTS flush (remaining): \"{remaining}\"");
        let _ = tx_tts.send(remaining).await;
    }

    if full_response.is_empty() {
        anyhow::bail!("Ollama stream produced no response");
    }

    debug!("Full response: \"{}\"", full_response);

    // ── Update session history (text only — no images stored) ──
    history.push_user(user_text);
    history.push_assistant(&full_response);

    Ok(full_response)
}
