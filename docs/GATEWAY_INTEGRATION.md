# Gateway integration rules

The gateway contract is not yet frozen.

Before implementing:
1. Obtain the approved API contract from `universal-ai-gateway`.
2. Confirm base URL and authentication behavior.
3. Confirm model identifier format.
4. Confirm error schema and timeout expectations.
5. Confirm streaming protocol and cancellation behavior.

Do not assume that OpenAI-compatible means every OpenAI endpoint or field is supported. Implement only the documented subset and test against the gateway.
