# ZipChat

A lightweight, standalone desktop AI chat application built in Rust with `egui` / `eframe`.

ZipChat connects directly to OpenAI-compatible hosted APIs (OpenAI, OpenRouter, Groq, Cerebras, Together, Mistral, DeepSeek, Gemini OpenAI endpoint) and local model inference servers (Ollama, LM Studio, or Universal AI Gateway as a preset).

## Principles

- **Standalone Native Application**: Direct communication with any OpenAI-compatible API; no mandatory gateway proxy, browser runtime, or heavy webviews.
- **Privacy & Security**: Credentials and conversations remain strictly local. API keys are kept out of settings files, logs, and exports.
- **Resource-Conscious & Fast**: Efficient and lightweight, optimized for modest hardware including Raspberry Pi OS (ARM64).
- **Responsive UI**: Real-time token SSE streaming, connection state visibility, accessible keyboard navigation, and smart auto-scrolling.
- **Atomic Local Persistence**: Reliable, crash-resilient local storage in standard platform configuration directories.

## Supported Providers & Presets

- OpenAI (`https://api.openai.com/v1`)
- OpenRouter (`https://openrouter.ai/api/v1`)
- Groq (`https://api.groq.com/openai/v1`)
- Cerebras (`https://api.cerebras.ai/v1`)
- Together AI (`https://api.together.xyz/v1`)
- Mistral (`https://api.mistral.ai/v1`)
- DeepSeek (`https://api.deepseek.com/v1`)
- Google Gemini OpenAI Endpoint (`https://generativelanguage.googleapis.com/v1beta/openai/`)
- Ollama (`http://localhost:11434/v1`)
- LM Studio (`http://localhost:1234/v1`)
- Universal AI Gateway (`http://127.0.0.1:9471/v1`)
- Custom OpenAI-compatible endpoints
