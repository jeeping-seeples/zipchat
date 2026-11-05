# Decisions

## ZD-001 — Separate client repository
ZipChat is a standalone client. Gateway provider integrations, credentials, and routing do not belong here.

## ZD-002 — Gateway is the source of truth
The gateway's approved API contract determines routes, request/response schemas, streaming format, model identifiers, and authentication.

## ZD-003 — Incremental human-steered development
Agents must plan, show progress, make small changes, and pause for approval.

## ZD-004 — No provider credentials in the client
ZipChat should not call model vendors directly in the initial architecture.

## ZD-005 — Framework and UI toolkit: Rust + egui / eframe
ZipChat is developed as a native desktop application using Rust and `eframe` / `egui`. This provides a lightweight, cross-platform UI suitable for desktop without webview or heavyweight browser runtimes.

## ZD-006 — v1 Gateway Contract: Non-streaming HTTP
Initial client communication integrates with Universal AI Gateway's confirmed endpoints: `GET /health`, `GET /v1/models`, and non-streaming `POST /v1/chat/completions`. Streaming and authentication are deferred until implemented in the gateway.

## ZD-007 — Multi-turn conversation context and in-memory management
ZipChat maintains multiple named conversation sessions in memory with full context history sent to the gateway for coherent chat turns, individual message status badges, and failure recovery with retry.

## ZD-008 — Server-Sent Events (SSE) streaming and usability
ZipChat supports SSE streaming matching the gateway's live contract (`text/event-stream`, `data: <JSON>`, `data: [ERROR: ...]`, `data: [DONE]`) with buffered network chunk parsing and graceful `application/json` fallback. Streaming is toggleable in settings (default off). Client supports immediate abort via "Stop Generating" or Escape, smart auto-scrolling with user scroll pause, and keyboard shortcuts.

## ZD-009 — Local atomic JSON persistence and conversation management
ZipChat saves settings (`settings.json`) and conversation history (`conversations.json`) separately in standard platform directories (`~/.config/zipchat` on Linux/Raspberry Pi, `~/Library/Application Support/zipchat` on macOS, `%APPDATA%\zipchat` on Windows, falling back to `./.zipchat`). All disk operations use atomic file replacement (temp file + rename) to protect data integrity against crashes. Active conversations can be exported to clean Markdown (`.md`) and JSON (`.json`), and individual or all conversations can be deleted safely with confirmation.
