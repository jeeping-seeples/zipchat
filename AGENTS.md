# Agent instructions

1. Read `README.md`, `PLAN.md`, `DECISIONS.md`, `PROGRESS.md`, `BLOCKERS.md`, and `docs/` before work.
2. Begin with approved tasks per `PLAN.md`.
3. Adhere to OpenAI-compatible provider endpoints: `GET {base}/models` and `POST {base}/chat/completions`.
4. Store provider credentials securely (OS keyring or 0600 file); never commit credentials, log them, or write keys to `settings.json` or exports.
5. Make small, reviewable changes and report files changed, commands run, test outcomes, and blockers.
6. Ask before adding dependencies, changing UI frameworks, changing persisted conversation/settings formats, or introducing telemetry.
7. Never commit credentials or `.env` files.
8. Pause for human approval after each task.
