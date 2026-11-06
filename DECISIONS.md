# Decisions

## ZD-001 — Separate client repository (Superseded by ZD-010)
*Originally established ZipChat as a thin client for Universal AI Gateway.*

## ZD-002 — Gateway is the source of truth (Superseded by ZD-010)
*Originally required gateway's approved API contract as the single provider interface.*

## ZD-003 — Incremental human-steered development
Agents must plan, show progress, make small changes, and pause for approval.

## ZD-004 — No provider credentials in the client (Superseded by ZD-010 & ZD-012)
*Originally prohibited provider keys in ZipChat.*

## ZD-005 — Framework and UI toolkit: Rust + egui / eframe
ZipChat is developed as a native desktop application using Rust and `eframe` / `egui`. This provides a lightweight, cross-platform UI suitable for desktop without webview or heavyweight browser runtimes.

## ZD-006 — v1 Gateway Contract: Non-streaming HTTP (Superseded by ZD-010 & ZD-011)
*Originally integrated with gateway-specific non-streaming endpoints.*

## ZD-007 — Multi-turn conversation context and in-memory management
ZipChat maintains multiple named conversation sessions in memory with full context history sent for coherent chat turns, individual message status badges, and failure recovery with retry.

## ZD-008 — Server-Sent Events (SSE) streaming and usability
ZipChat supports SSE streaming matching OpenAI-compatible streaming contracts (`text/event-stream`, `data: <JSON>`, `data: [DONE]`) with buffered network chunk parsing. Client supports immediate abort via "Stop Generating" or Escape, smart auto-scrolling with user scroll pause, and keyboard shortcuts.

## ZD-009 — Local atomic JSON persistence and conversation management
ZipChat saves settings (`settings.json`) and conversation history (`conversations.json`) separately in standard platform directories (`~/.config/zipchat` on Linux/Raspberry Pi, `~/Library/Application Support/zipchat` on macOS, `%APPDATA%\zipchat` on Windows, falling back to `./.zipchat`). All disk operations use atomic file replacement (temp file + rename). Active conversations can be exported to clean Markdown (`.md`) and JSON (`.json`).

## ZD-010 — Standalone Direct Provider Architecture (Supersedes ZD-001, ZD-002, ZD-004, ZD-006)
ZipChat pivots to a standalone AI chat desktop application connecting directly to any OpenAI-compatible provider. `universal-ai-gateway` remains supported as an optional provider preset, but is no longer a required dependency.

## ZD-011 — OpenAI-Compatible API Surface
ZipChat standardizes on the standard OpenAI HTTP API:
- `GET {base}/models` for model discovery and connection health tests.
- `POST {base}/chat/completions` for chat requests and SSE streaming.
Base URLs include the version prefix where applicable (e.g., `https://api.openai.com/v1`, `http://localhost:11434/v1`).

## ZD-012 — API Key Security and Credential Handling
Provider API keys are handled securely by ZipChat:
- Keys are never committed to version control, logged, or exported.
- Keys are redacted from `Debug` representations.
- Keys are kept out of `settings.json` and stored in the OS keyring where available, with a fallback `0600` permissions secrets file on headless Linux / Raspberry Pi environments.
