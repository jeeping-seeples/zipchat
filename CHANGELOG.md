# Changelog

## 0.4.0 - Phase 4 (Local Persistence & Polish)
- Added `StorageManager` providing atomic JSON persistence in platform config directories (`~/.config/zipchat` on Linux/Raspberry Pi OS, macOS, and Windows).
- Added separate storage files for `settings.json` and `conversations.json`.
- Automatic load on launch and auto-save on conversation/settings changes.
- Added per-conversation delete button and "Clear All History" modal confirmation dialog.
- Added export to clean Markdown (`.md`) and formatted JSON (`.json`) with in-app confirmation notifications.
- Fully functional offline browsing and management of conversations.
- Added `docs/PACKAGING.md` with build guides for Linux x86_64, Raspberry Pi OS ARM64, macOS, Windows, and `.deb`/AppImage packaging.
- Added automated storage unit/integration tests (`tests/storage_tests.rs`).

## 0.3.0 - Phase 3 (Streaming & Usability)
- Added Server-Sent Events (SSE) streaming support for `/v1/chat/completions`.
- Handled split network buffers, comment lines, `[ERROR: ...]` mid-stream events, and non-streaming `application/json` fallback.
- Added streaming toggle in settings (default OFF).
- Added real-time token rendering for assistant messages.
- Added "Stop Generating" button and Escape key shortcut to cancel in-flight generation while preserving partial text.
- Added Escape-to-clear input when idle, Enter-to-send, and Shift+Enter for newline.
- Added auto-scroll to bottom with automatic pause on user scroll-up and settings toggle (default ON).
- Added comprehensive mock tests for SSE streaming, split chunks, errors, fallback, and cancellation.

## 0.2.0 - Phase 2 (Basic Chat)
- Implemented multi-turn chat completions dispatching entire conversation history.
- Added multiple concurrent in-memory conversations with active selection and "New Chat" creation.
- Designed two-column desktop interface: collapsible sidebar and main chat panel.
- Added message status tracking (`Sending`, `Delivered`, `Failed`) with one-click `Retry` on error.
- Supported multiline text composer with Enter-to-send and Shift+Enter for newlines.
- Added automated mock integration test for multi-turn conversations.

## 0.1.0 - Phase 1 (App Shell & Connection Test)
- Configured Rust package with `eframe`, `egui`, `tokio`, `reqwest`, and `wiremock`.
- Implemented `GatewayClient` supporting `/health`, `/v1/models`, and `/v1/chat/completions`.
- Added automated mock integration tests for client API endpoints and error handling.
- Built native desktop App Shell with base URL configuration, status indicators, and model selection.
