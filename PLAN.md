# Project plan

Status: planning; gateway contract not yet frozen.

## Phase 0 — Read-only client feasibility audit

- Inspect current repository contents.
- Confirm intended platforms and UI toolkit from existing project evidence; do not assume.
- Recommend a clean separation between UI, gateway client, state, and persistence.
- Identify a minimal conversation data model.
- Explain connection settings and safe secret storage options per target platform.
- Define loading, error, empty, and streaming UI states.
- Identify accessibility and small-screen considerations.
- Check the gateway repository's approved API contract if accessible; if unavailable, list it as a blocker rather than inventing one.

**Exit criteria:** human approves the design and confirms target platforms.

## Phase 1 — App shell

- Establish buildable application shell using the approved framework.
- Add settings screen for gateway base URL.
- Add connection test using the approved health endpoint.
- Add visible connection and error states.

## Phase 2 — Basic chat

- Implement conversation list and message view.
- Send a message through the gateway's approved endpoint.
- Render user and assistant messages with clear status.
- Handle timeout, cancellation, and server errors.

## Phase 3 — Streaming and usability

- Add streaming only after gateway format is confirmed.
- Add stop/cancel behavior if supported by the API.
- Add model selection from the gateway model list.
- Add accessible keyboard and touch interactions as applicable.

## Phase 4 — Persistence and polish

- Add local conversation persistence with an explicit data model.
- Define export/delete behavior.
- Add tests and platform-specific packaging documentation.

## Release readiness

- Verify compatibility against a running gateway.
- Document setup and troubleshooting.
- Confirm no provider credentials are required in ZipChat.
