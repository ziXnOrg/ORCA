# ORCA API How-To

Covers common workflows using the gRPC API defined in `Docs/API/orca_v1.proto`.

## Envelope
Fields (client-supplied unless noted):
- `id`: unique message id (idempotency)
- `parent_id`: parent message id or empty
- `trace_id`: trace/run correlation id
- `agent`: producer identifier (e.g., name/version)
- `kind`: semantic kind (e.g., `agent_task`, `agent_result`)
- `payload_json`: JSON string payload
- `timeout_ms`: per-task timeout enforced by orchestrator
- `protocol_version`: current protocol version (see `Docs/API/versioning.md`)
- `ts_ms`: client timestamp (ms)
- `usage`: optional usage hints `{ tokens, cost_micros }` captured from SDK/tool

## Start a run
- RPC: `StartRun(StartRunRequest)` with optional `initial_task: Envelope` and optional `budget: Budget`
- Policy pre-hook may redact or deny; on allow it is WAL-appended.

## Submit a task
- RPC: `SubmitTask(SubmitTaskRequest)` with `task: Envelope`
- Idempotency: duplicate `Envelope.id` is deduped.
- Budget checks may reject with `RESOURCE_EXHAUSTED`.
- Policy post-hook may gate emission.

## Stream events
- RPC: `StreamEvents(StreamEventsRequest)`
- Use `start_event_id` or `since_ts_ms` to tail from a point-in-time.
- Backpressure: the server applies flow control; client should consume promptly.

## Fetch result
- RPC: `FetchResult(FetchResultRequest)` for terminal outputs if supported.

## Budgets & Cost
- Configure per-run budgets via `StartRun.budget`, or via env defaults `ORCA_MAX_TOKENS`, `ORCA_MAX_COST_MICROS`.
- See `Docs/cost_management.md` for details on tracking, thresholds, and error handling.

## Security
- Auth: send `authorization: Bearer <token>` metadata.
- TLS/mTLS: see `Docs/security.mtls.md`; provide CA to SDKs.

## Errors
- `PERMISSION_DENIED`: policy or auth failures
- `RESOURCE_EXHAUSTED`: budget exceeded or backpressure limits
- `INVALID_ARGUMENT`: malformed request
- `DEADLINE_EXCEEDED`: `timeout_ms` exceeded
