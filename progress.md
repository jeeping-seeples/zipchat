# Progress Log

## Session: 2026-11-08

### Phase 1: Fix Settings Modal Close Bug & Model Visibility UX
- **Status:** complete
- **Actions taken:**
  - Initialized `planning-with-files` (`task_plan.md`, `findings.md`, `progress.md`).
  - Diagnosed root cause of unclosable Settings window in `src/app.rs`: missing `.open(&mut modal_open)` on `egui::Window`, unconstrained `ScrollArea::vertical()` pushing the bottom `Close` button off-screen, rigid `Align2::CENTER_CENTER` anchor preventing window dragging, and no `Escape` key handler.
  - Fixed Settings window in `src/app.rs` by adding `.open(&mut modal_open)`, a top-right `✕ Close` button in the tab bar, `Escape` key dismissal, movable window positioning, and `.max_height()` bounds on inner scroll areas.
  - Replaced the growing blacklist tag cloud with an in-place searchable model visibility table: filter input (`🔍 Filter`), `Hide Matching` bulk button, `Show Hidden` toggle checkbox, `Unhide All` button, and per-row visibility checkboxes.
  - Verified formatting with `cargo fmt --all -- --check` (exit 0).

### Phase 2: Fix Vercel Gateway Errors & Message Ergonomics
- **Status:** complete
- **Actions taken:**
  - Diagnosed Vercel error causes:
    1. Unconditionally injecting `serviceTier: "flex"` caused Vercel to reject requests for `openai/gpt-4o` because `flex` is only supported on select reasoning models (`o3-mini`, etc.).
    2. Missing provider prefix (e.g. `gpt-4o` vs `openai/gpt-4o`) causes 400 Bad Request on Vercel AI Gateway.
    3. `ApiClient::extract_error` previously failed on non-standard JSON or plain text errors from gateways, masking them as generic "HTTP error 400".
  - Implemented `parse_error_body` in `src/client.rs` to extract messages from nested objects, string error values, and raw responses.
  - Added model auto-prefixing (`openai/`, `anthropic/`, `google/`) for Vercel in `src/app.rs`.
  - Added `service_tier()` and `set_service_tier()` on `Provider` with a UI dropdown in Provider Settings (`None (Default) | flex | priority`), removing the hardcoded `flex` tier default from `gpt-4o`.
  - Added message `📋 Copy`, user message `✎ Edit` (loads into composer), and message `🗑` Delete actions to chat timeline.
  - Enhanced message token breakdown (`X tokens (Y in, Z out)`).
  - Added unit test `test_chat_completion_string_error_handling` in `tests/client_tests.rs`.
  - Verified formatting with `cargo fmt --all -- --check` (exit 0).

### Phase 3: Sidebar & Conversation Management
- **Status:** complete
- **Actions taken:**
  - Added search input `🔍 Search chats...` in sidebar matching both conversation titles and message content.
  - Added inline conversation renaming with `✎` button, `✔` confirmation, and Enter key submit.
  - Added collapsible sidebar toggle with `☰` button in chat header, `◀` collapse button in sidebar, and `Ctrl+B` shortcut.
  - Added `last_activity_secs()` and `updated_at_epoch_secs` on `Conversation`, sorting conversations by recent activity.
  - Added unit test `test_conversation_rename_and_activity_order` in `tests/storage_tests.rs`.
  - Verified formatting with `cargo fmt --all -- --check` (exit 0).

### Phase 4: Keyboard Shortcuts & Composer Polish
- **Status:** complete
- **Actions taken:**
  - Added global keyboard shortcuts in `src/app.rs`:
    - `Esc`: closes modals/confirmations or stops active generation.
    - `Ctrl+B`: toggles sidebar collapse/expand.
    - `Ctrl+N`: creates a new conversation.
    - `Ctrl+,`: opens/closes Settings modal.
  - Implemented auto-expanding multiline composer (dynamically scaling from 1 to 8 lines based on input length).
  - Added inline `⏹ Stop` button right in the composer during generation, alongside `Send ↵` and a quick `Clear` button.
  - Verified formatting with `cargo fmt --all -- --check` (exit 0).

### Phase 5: Provider & Model Selector Enhancements
- **Status:** complete
- **Actions taken:**
  - Added dynamic status dots (`●`) with hover tooltips in chat header, sidebar Quick Settings, and Manage Providers list (green/online, yellow/refreshing, red/error, gray/disabled).
  - Added `Sort by Price` toggle checkbox in the model selector dropdown.
  - Added unit test `test_provider_status_dot` in `tests/storage_tests.rs`.
  - Verified formatting with `cargo fmt --all -- --check` (exit 0).
