# Proposed Settings Schema & Migration Plan (Pivot Phase 0)

## 1. Proposed `settings.json` Schema

```json
{
  "version": 2,
  "default_provider_id": "openai",
  "default_model": "gpt-4o",
  "stream_enabled": true,
  "auto_scroll_enabled": true,
  "providers": [
    {
      "id": "openai",
      "name": "OpenAI",
      "base_url": "https://api.openai.com/v1",
      "enabled": true,
      "models": [
        "gpt-4o",
        "gpt-4o-mini"
      ],
      "models_fetched_at": 1700000000
    },
    {
      "id": "openrouter",
      "name": "OpenRouter",
      "base_url": "https://openrouter.ai/api/v1",
      "enabled": true,
      "models": [],
      "models_fetched_at": null,
      "model_pricing": {
        "openai/gpt-4o": "in: $2.50/M · out: $10.00/M"
      },
      "model_blacklist": ["whisper", "embed"]
    },
    {
      "id": "vercel",
      "name": "Vercel AI Gateway",
      "base_url": "https://ai-gateway.vercel.sh/v1",
      "enabled": true,
      "models": [
        "openai/gpt-4o"
      ],
      "models_fetched_at": null,
      "providerOptions": {
        "gateway": {
          "serviceTier": "flex"
        }
      },
      "model_pricing": {},
      "model_blacklist": []
    },
    {
      "id": "ollama",
      "name": "Ollama (Local)",
      "base_url": "http://localhost:11434/v1",
      "enabled": true,
      "models": [
        "llama3.2:latest"
      ],
      "models_fetched_at": 1700000000
    },
    {
      "id": "gateway",
      "name": "Universal AI Gateway",
      "base_url": "http://127.0.0.1:9471/v1",
      "enabled": false,
      "models": [],
      "models_fetched_at": null
    }
  ]
}
```

### Key Field Descriptions:
- **`version`**: Schema format version (`2`).
- **`providers`**: Ordered list of configured providers.
  - `id`: Unique slug identifier (e.g. `openai`, `groq`, `custom-1`).
  - `name`: Display name in UI.
  - `base_url`: Full API base URL including `/v1` or equivalent path prefix.
  - `enabled`: Toggle for whether this provider appears in chat model pickers.
  - `models`: Cached list of model IDs pulled via `GET {base_url}/models`.
  - `models_fetched_at`: Unix epoch seconds timestamp of last successful model discovery.
- **`default_provider_id`** & **`default_model`**: Initial selection for newly created conversations.
- **Security**: API keys are **never** stored in `settings.json`. Keys are saved in OS Keyring or `secrets.json` (mode 0600).

---

## 2. Legacy Migration Plan

ZipChat previously stored `settings.json` in v1 format:
```json
{
  "gateway_url": "http://127.0.0.1:9471",
  "selected_model": "mock-gpt",
  "stream_enabled": false,
  "auto_scroll_enabled": true
}
```

### Migration Rules:
1. When loading `settings.json`, inspect whether `version` is missing or `gateway_url` is present.
2. If v1 is detected:
   - Convert `gateway_url` into a provider named `"Universal AI Gateway"` with id `"gateway"` and `base_url = "{gateway_url}/v1"`.
   - Add default presets (OpenAI, OpenRouter, Groq, Ollama, etc.) with enabled flags.
   - Set `default_provider_id = "gateway"`.
   - Set `default_model = selected_model`.
   - Persist upgraded v2 `settings.json`.
3. Legacy `conversations.json` compatibility:
   - If a conversation has `model: Some("mock-gpt")` and no `provider_id`, assign `provider_id = "gateway"`.
   - All conversation histories and message states load without data loss.
