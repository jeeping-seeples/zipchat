# Findings & Research

## Technical Discoveries

### 1. Root Cause: Settings Window Cannot Be Closed
In `src/app.rs` (lines 1662–1898):
- `egui::Window::new("⚙ Settings")` does not use `.open(&mut is_open)`, so egui does not render a close (`✕`) button in the window title bar.
- The `ScrollArea::vertical()` inside `SettingsTab::Providers` (line 1690) and `SettingsTab::Models` (line 1812) has no `.max_height(...)` constraint. In `egui`, an unconstrained vertical `ScrollArea` inside a `Window` expands to fill or exceed the window height, pushing the bottom `ui.separator()` and `Close` button (line 1893) off the bottom edge of the window.
- Because `.anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])` is set on the window, the user cannot drag the window upward to reach the overflowed bottom `Close` button.
- Pressing `Escape` is not wired to close `self.show_settings_modal`.

### 2. Model Blacklist UX Issue
- Rendering blacklisted patterns as a horizontal wrapped list of tag buttons (`✕ pattern`) grows unboundedly as users hide models or patterns, consuming vertical space in the `Models` tab and further exacerbating the scroll overflow issue.
- Better UX:
  - Keep a single search/filter bar in the `Models` tab (`settings_model_filter`).
  - Include a `"Show Hidden"` checkbox toggle so users can inspect or hide blacklisted models in the existing scrollable list.
  - Provide bulk actions right next to the filter input (`Hide Matching` / `Unhide Matching` / `Reset Hidden`) plus a per-row visibility checkbox/button in the scrollable model table. Zero growing tag clouds.

### 3. UI & UX Improvement Opportunities Across `src/app.rs`
- **Message Actions**: Currently messages only show `Retry` (on failure) and `↺ Regenerate` (on last assistant response). Missing: `Copy` message to clipboard (`ui.ctx().copy_text`), `Copy Code` on fenced code blocks in `render_message_content`, and `Delete` individual message.
- **Sidebar Navigation**: Conversation list lacks search/filtering and inline title renaming (`✎`).
- **Keyboard Shortcuts**: No global hotkeys for `Esc` (close modals / cancel stream), `Ctrl+N` (new chat), or `Ctrl+,` (toggle settings).
- **Composer**: Fixed `48.0` height (`ui.add_sized([ui.available_width() - 80.0, 48.0], ...)`). Using `desired_rows(2)` with dynamic growth or cleaner layout improves prompt editing.
- **Model Selector**: Shows all models unsorted; sorting alphabetically with pinned at top and showing concise pricing improves scanability.

## Constraints & Architecture Notes
- Must adhere to `AGENTS.md`: no new dependencies without approval, preserve `settings.json` / `conversations.json` compatibility (`#[serde(default)]`), keep resource usage low for ARM64 / Raspberry Pi 4B.
- CI pipeline (`ZipChat ARM64 CI`) enforces `cargo fmt --all -- --check`, `cargo test --locked --all-targets`, and `cargo clippy --locked --all-targets -- -D warnings`.
