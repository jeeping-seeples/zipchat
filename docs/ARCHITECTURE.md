# Proposed client architecture

This is a hypothesis for the audit, not an approved design.

```text
UI (views / components)
       |
Application state and actions
       |
Gateway client (HTTP + DTOs)
       |
Universal AI Gateway
```

Possible responsibilities:
- **UI:** conversation list, message timeline, composer, settings, connection status.
- **Application state:** selected conversation/model, pending request, cancellation state.
- **Gateway client:** base URL, HTTP requests, response decoding, streaming parser after contract approval.
- **Persistence:** local conversations and preferences, with a documented schema.

Keep network code out of view components. Keep provider-specific logic out of ZipChat.
