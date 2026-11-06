# Gateway Integration (Provider Preset)

Following ZipChat's pivot to a standalone AI chat application (ZD-010), `universal-ai-gateway` is supported as an optional provider preset rather than a mandatory client-server dependency.

## Preset Configuration

- **Provider Name:** Universal AI Gateway
- **Base URL:** `http://127.0.0.1:9471/v1`
- **Authentication:** Optional (empty key by default for localhost)
- **Endpoints:**
  - `GET http://127.0.0.1:9471/v1/models`
  - `POST http://127.0.0.1:9471/v1/chat/completions`
