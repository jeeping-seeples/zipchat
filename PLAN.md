# Project plan

Status: pivoting. ZipChat was scaffolded as a thin client for `universal-ai-gateway`. It is now a standalone AI chat app that talks directly to OpenAI-compatible providers.

The old Phases 0-4 (app shell, basic chat, SSE streaming, atomic persistence, export, packaging) are done and are the baseline. This plan changes what sits behind them.

## Direction

- ZipChat is a lightweight, standalone Rust + egui/eframe AI chat app.
- The user configures providers (name, base URL, API key). ZipChat calls them directly through the OpenAI-compatible API: `GET {base}/models` and `POST {base}/chat/completions`.
- `universal-ai-gateway` stays supported as one provider preset. It is no longer a dependency.
- This supersedes ZD-001, ZD-002, ZD-004 and ZD-006, and the "no provider credentials in the client" rules in README.md and AGENTS.md.

Out of scope for v1: native non-OpenAI APIs (e.g. Anthropic `/v1/messages`), tool calling, image input, accounts or sync.

## Phase 0 - Pivot decisions (docs only, no code)

- Add decision entries: ZipChat calls providers directly (supersedes ZD-001/002/004/006); OpenAI-compatible only for v1; API key storage approach.
- Rewrite README.md principles and AGENTS.md rules 3 and 4 to match. Mark docs/GATEWAY_INTEGRATION.md as "gateway is one provider preset".
- Draft the new `settings.json` schema and migration plan for approval (AGENTS.md rule 6 requires approval before persisted-format changes).

**Exit criteria:** human approves the decisions and the settings schema.

## Phase 1 - Provider model and settings

- Data model: `Provider { id, name, base_url, enabled, models, models_fetched_at }`. Settings hold `Vec<Provider>` plus the default provider and model.
- Migration: on first load, a legacy `gateway_url` becomes a "Gateway" provider. Existing settings.json and conversations.json must still load.
- Fix URL handling in `client.rs`: the base URL now includes the version path (e.g. `https://api.openai.com/v1`), so requests go to `{base}/models` and `{base}/chat/completions`. Remove the hard-coded `/v1` and the `/health` call; "Test connection" becomes a models-list request.
- Make the client per-provider instead of one global gateway client.
- Editable provider presets (verify each base URL against the provider's docs before shipping): OpenAI, OpenRouter, Groq, Cerebras, Together, Mistral, DeepSeek, Gemini (OpenAI-compatible endpoint), Ollama (`http://localhost:11434/v1`), LM Studio (`http://localhost:1234/v1`), Universal AI Gateway (`http://127.0.0.1:9471/v1`), plus a Custom option with any base URL.
- Settings UI: add, edit and remove providers; base URL field; masked key field; Test connection button.
- Clear connection errors: bad key (401/403), wrong base URL (404), timeout, DNS or connection refused.
- Tolerant parsing in `models.rs`: only `id` is required on a model entry; `object`, `created`, `owned_by` and `usage` become optional (`#[serde(default)]`); unknown fields ignored. Many providers omit these.

## Phase 2 - API keys

- Send `Authorization: Bearer <key>` when a key is set. An empty key is allowed for local servers.
- Keep keys out of settings.json, conversations.json, exports and logs; redact them in `Debug` output.
- Storage: OS keyring where available, with a fallback secrets file (permissions 0600) for headless Linux / Raspberry Pi. Ask before adding the keyring dependency.
- Optional per-provider extra headers (e.g. OpenRouter attribution headers).

## Phase 3 - Auto model pull

- Fetch `{base}/models` automatically when a provider is added or saved, and on app start in the background without blocking the UI.
- Cache the model list and fetch timestamp in settings; if a fetch fails, keep showing the cached list with a stale/offline indicator.
- Manual Refresh button per provider, with loading and error states.
- Model picker grouped by provider, with search/filter and pinned favorites.
- Manual "add model ID" fallback for providers whose `/models` endpoint is missing or incomplete.
- After a refresh, keep the currently selected model even if it disappeared from the list, but flag it.

## Phase 4 - Chat across providers

- Each conversation stores `provider_id` and `model` (migrate the old `model: Option<String>`). Switching model or provider mid-conversation works.
- System prompt, temperature and max tokens: global defaults with optional per-conversation override.
- Streaming on by default. The existing SSE parser stays; harden it for empty `choices` arrays, usage-only final chunks and providers that add extra delta fields.
- Regenerate and retry on the last response. Stop/Escape already exists.
- Map errors to readable messages: 401, 403, 404 (bad model or base URL), 429 (show retry-after if present), 5xx.

## Phase 5 - Polish

- Markdown and code-block rendering with a copy button.
- Optional auto chat titles using a fast model the user picks in settings.
- Show token usage when the provider returns it.
- Mock-server tests covering: model list parsing variants, auth header, streaming, error mapping, settings migration.
- Update docs/PACKAGING.md and add provider setup docs.

## Release readiness

- Verify against one hosted provider, one aggregator (OpenRouter-style) and one local server (Ollama or LM Studio).
- Confirm keys never appear in logs, exports or settings.json.
- Document setup and troubleshooting.
