# Decisions

## ZD-001 — Separate client repository
ZipChat is a standalone client. Gateway provider integrations, credentials, and routing do not belong here.

## ZD-002 — Gateway is the source of truth
The gateway's approved API contract determines routes, request/response schemas, streaming format, model identifiers, and authentication.

## ZD-003 — Incremental human-steered development
Agents must plan, show progress, make small changes, and pause for approval.

## ZD-004 — No provider credentials in the client
ZipChat should not call model vendors directly in the initial architecture.
