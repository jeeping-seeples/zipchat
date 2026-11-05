# ZipChat

A separate, lightweight chat client for the Universal AI Gateway.

ZipChat owns the user experience: conversations, model selection, settings, and presentation. It does not own provider API integrations or routing logic; those belong in `universal-ai-gateway`.

## Principles

- Rust application; current UI toolkit and platform targets must be confirmed by the audit.
- Consume the gateway's documented API rather than inventing endpoints.
- Keep the client responsive and resource-conscious.
- Make connection state, errors, and streaming state visible.
- Keep secrets out of logs and source control.
- Build incrementally with human approval at each task boundary.

## Dependency

Gateway repository: `https://github.com/jeeping-seeples/universal-ai-gateway`

The gateway API is not frozen yet. Do not implement network calls until its contract is confirmed.
