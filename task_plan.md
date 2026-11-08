# Task Plan: ZipChat UI/UX Improvements & Settings Modal Fix

## Goal
Fix the unclosable Settings modal, resolve Vercel AI Gateway chat errors, and establish/deliver structured UI, UX, and feature improvements in ZipChat across all 5 planned phases.

## Next Step
Deliver summary to user and propose commit/push of the completed roadmap.

## Current Phase
Phase 5

## Phases

### Phase 1: Fix Settings Modal Close Bug & Model Visibility UX
- [x] Diagnose why Settings window cannot be closed (unbounded `ScrollArea` pushing bottom `Close` button off-screen, missing `.open()`, rigid center anchor, no `Esc` key handler)
- [x] Document findings in `findings.md`
- [x] Add `.open()` titlebar close button, top-bar `✕ Close` button, `Esc` key dismissal, and bounded `max_height` on modal `ScrollArea`s
- [x] Replace growing blacklist tag cloud with an in-place searchable model table featuring per-row visibility checkboxes, a `Show Hidden` toggle, and bulk `Hide Matching` / `Unhide All` actions
- **Status:** complete

### Phase 2: Fix Vercel Gateway Errors & Message Ergonomics
- [x] Diagnose and fix Vercel AI Gateway errors:
  - Add robust `parse_error_body` in `ApiClient::extract_error` so error messages (JSON objects, string error values, raw HTTP snippets) are never masked as generic "HTTP error 400"
  - Auto-prefix model names (e.g. `gpt-4o` -> `openai/gpt-4o`) when targeting Vercel
  - Stop forcing `serviceTier: "flex"` on models that don't support it (like `gpt-4o`), and add a Service Tier selector (`None (Default) | flex | priority`) in Provider Settings
- [x] Add message `📋 Copy` button to message headers
- [x] Add `✎ Edit` button to user messages (loads message into composer for quick editing and resending)
- [x] Add `🗑` Delete message action to remove individual messages from history
- [x] Display detailed prompt & completion token breakdown in message header (`total tokens (X in, Y out)`)
- [x] Add unit test for flexible gateway error extraction (`test_chat_completion_string_error_handling`)
- **Status:** complete

### Phase 3: Sidebar & Conversation Management
- [x] Add conversation search/filter input in the left sidebar (`🔍 Search chats...`) with title and message content search
- [x] Add inline conversation rename (`✎` / `✔` / `✕`) with Enter key submission
- [x] Add collapsible sidebar toggle (`☰` in header, `◀` in sidebar) and `Ctrl+B` shortcut
- [x] Sort conversations by recent activity (`last_activity_secs`) with automatic timestamp updates
- [x] Add unit test for conversation renaming and activity sorting (`test_conversation_rename_and_activity_order`)
- **Status:** complete

### Phase 4: Keyboard Shortcuts & Composer Polish
- [x] Implement global shortcuts: `Esc` (close modals / cancel generation), `Ctrl+N` (new conversation), `Ctrl+,` (toggle settings modal), `Ctrl+B` (toggle sidebar)
- [x] Upgrade composer with auto-expanding multiline rows (from 1 to 8 lines dynamically)
- [x] Add inline `⏹ Stop` button in composer during generation, alongside `Send ↵` and quick `Clear` button
- **Status:** complete

### Phase 5: Provider & Model Selector Enhancements
- [x] Add dynamic provider status dot indicators (`●` green/online, yellow/refreshing, red/error, gray/disabled) in chat header, Quick Settings, and Manage Providers list
- [x] Add model sorting toggle (`Sort by Price` / alphabetical) in model selector dropdown
- [x] Add Provider Service Tier selector (`None (Default) | flex | priority`) for Vercel/gateway providers
- [x] Add unit test for provider status dot states (`test_provider_status_dot`)
- **Status:** complete

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Bound modal `ScrollArea` heights + add `.open()` and top-right `✕ Close` | Prevents inner scroll areas from pushing modal controls off-screen and provides standard window close affordances |
| Replace blacklist tag chips with searchable list + per-row visibility checkboxes + bulk Hide/Unhide | Eliminates unbounded UI growth when blacklisting many models while making bulk hiding (e.g. all `embed` or `whisper` models) effortless |
| Parse raw error bodies and allow configurable `serviceTier` | Vercel rejects `flex` tier for models like `gpt-4o`; surfacing real server errors and making `serviceTier` optional eliminates 400 errors |
| Add `updated_at_epoch_secs` & activity sorting to `Conversation` | Most active/recent conversations stay at the top for immediate access; backward-compatible via `#[serde(default)]` |
| Auto-expanding composer + inline Stop/Clear controls | Adapts to user input length without wasting vertical space; places primary generation controls right where the user is typing |
| Dynamic status dots & Price sorting | Provides immediate visual feedback on provider health and cost optimization across multiple LLM backends |

## Errors Encountered
| Error | Resolution |
|-------|------------|
| Settings modal Close button unreachable & no titlebar X | Added `.open(&mut ...)`, top-bar `✕ Close` button, `Esc` handler, removed rigid anchor, and added `.max_height()` on modal scroll areas |
| Vercel chat 400 error when messaging with API key | Fixed unconditional `serviceTier: "flex"` payload on `gpt-4o`, added auto-prefixing for Vercel models (`openai/`), and added `parse_error_body` to expose real error strings |
