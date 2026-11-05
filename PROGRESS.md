# Progress

## Current status

- [x] Repository purpose and gateway/client boundary documented.
- [x] Read-only feasibility audit completed.
- [x] Target platforms and UI toolkit confirmed (Rust + egui/eframe).
- [x] Gateway API contract approved.
- [x] Buildable application shell.
- [x] Gateway connection test.
- [x] Basic chat.
- [x] Streaming.
- [x] Persistence and release documentation.

## Phase 4 Completed
- Modular `StorageManager` in `src/storage.rs` implementing standard platform configuration directories (Linux/Raspberry Pi OS `$XDG_CONFIG_HOME/zipchat`, macOS `~/Library/Application Support/zipchat`, Windows `%APPDATA%\zipchat`, and fallback `./.zipchat`).
- Safe atomic file replacement (temp file + rename) for `settings.json` and `conversations.json`.
- Automatic loading on startup and automatic saving on settings/conversation changes.
- Conversation deletion with trash icon and "Clear All History" with interactive confirmation dialog.
- Active conversation export to clean Markdown (`.md`) with code blocks and formatted JSON (`.json`) with in-app toast feedback.
- Offline support: viewing, managing, and exporting conversations requires zero network access.
- Cross-platform packaging guide in `docs/PACKAGING.md` with explicit Raspberry Pi ARM64 (aarch64) compilation, Debian package creation, and runtime dependency documentation.
- Comprehensive test coverage with 14 passing automated tests in `tests/`.
