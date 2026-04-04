# Cora AI 👁️

**Cora AI** is a lightweight, local, AI-powered gaming assistant. Built with Rust and Tauri, Cora provides a low-latency, fully private voice assistant that overlays directly on top of your games and desktop applications.

## ✨ Features

- **Wake Word Activation:** Simply say "Cora" to activate the assistant.
- **Local Speech-to-Text (STT):** High-performance, offline transcription using local Whisper/Vosk models.
- **Speculative Vision:** Automatically captures context (screenshots) upon activation and leverages local Vision LLMs to "see" your screen.
- **Interactive HUD Overlay:** A beautiful, transparent overlay built with React and Tauri, featuring a dynamic Eye sphere that reacts to different states (Idle, Listening, Thinking, Speaking).
- **Responsive LLM Pipeline:** Powered locally via Ollama (e.g., Llama 3 Vision) for maximum performance and privacy.
- **Fast Text-to-Speech (TTS):** Generates responses dynamically without relying on cloud services.

## 🛠️ Technology Stack

- **Core & Backend:** Rust + Tauri (for low CPU/RAM usage and native system integration)
- **Frontend UI:** React + TypeScript + Vite + Custom CSS
- **Local AI Engines:** Ollama (LLM/Vision), Whisper.cpp / Vosk (STT)

## 🚀 Getting Started

### Prerequisites

Ensure you have the following installed to run Cora AI locally:
- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/)
- [Ollama](https://ollama.com/) (running in the background)
- Required AI models downloaded (e.g., `llama3.2-vision` in Ollama, plus Vosk/Whisper STT binaries in your required paths)

### Installation & Development

1. Run `npm install` to install frontend dependencies.
2. Run `npm run tauri dev` to start the development environment.

## ⌨️ Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
