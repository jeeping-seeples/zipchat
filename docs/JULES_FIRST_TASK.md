# Jules task 001 — Read-only client architecture audit

## Mode: READ ONLY

Do not edit, create, delete, or rename source files. Do not add dependencies or commit changes. Return an audit report in your response only.

## Context

ZipChat is a separate lightweight chat client for `universal-ai-gateway`. The gateway owns provider integrations, credentials, routing, and orchestration. ZipChat consumes the gateway's approved HTTP API. The user values visible progress, incremental changes, and human approval—not unrestricted autonomy.

There may be an intended egui/Rust direction, but inspect the repository and report evidence before treating it as confirmed. Do not silently switch frameworks.

## Investigate

1. Inspect the repository and report what currently exists.
2. Identify build system, language, UI toolkit, and supported targets from evidence.
3. Recommend module boundaries for UI, application state, gateway client, and persistence.
4. Identify the smallest useful first application milestone.
5. Define expected UI states: empty, connecting, connected, loading, streaming, error, and cancelled where supported.
6. Review accessibility and responsive layout needs for the actual target platforms.
7. Review safe handling of gateway URL and authentication material.
8. Check for the gateway's approved API contract. If unavailable, make it an explicit blocker; do not invent endpoints or schemas.
9. Recommend a test strategy, including a mock HTTP server.
10. Identify risks, unresolved questions, and deferred features.

## Deliverable

Return:
- Executive recommendation.
- Evidence-based current-state summary.
- Proposed module tree.
- UI/state model.
- Gateway integration plan (only against a confirmed contract).
- Dependency recommendations and trade-offs.
- Test strategy.
- Phased plan with acceptance criteria.
- Questions requiring human decisions.

## Acceptance criteria

- No files changed and no dependencies added.
- No invented API endpoints or schemas.
- Provider credentials remain out of the client.
- Clearly distinguish evidence, assumptions, and recommendations.
- End by asking for human approval before implementation.
