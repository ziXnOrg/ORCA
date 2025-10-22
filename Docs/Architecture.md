# ORCA Architecture

## Overview
Developer-local AgentMesh runtime with deterministic, event-sourced execution and strong guardrails.

## Components
- Orchestrator (Rust): deterministic state machine; gRPC/HTTP2 transport (tonic)
- Event Log (WAL): JSONL append-only; replay-on-start; snapshots (see `Docs/snapshots.md`)
- Policy Engine: pre/post hooks for allow/deny/modify; redaction
- Budget Manager: counters and enforcement (tokens/cost/time)
- Telemetry: tracing/logs/metrics via `telemetry` crate and OTel
- Security: token auth, mTLS (rustls), RBAC (Casbin)
- SDKs: Python/TS clients via gRPC/proto

## Data Flow
1. Client sends `Envelope` (StartRun/SubmitTask) with auth metadata over TLS.
2. Pre-policy evaluates and may redact/deny/modify.
3. WAL append; in-memory index updates.
4. Execution/timeout; post-policy check; results emitted.
5. Streamers consume via `StreamEvents` with backpressure.

## Determinism & Recovery
- All state transitions are logged to WAL; restart replays to consistent point.
- Snapshots reduce replay time.

## Budgets & Guardrails
- Enforce cost/token/time limits early; fail closed.
- Backpressure and idempotency to protect runtime.

## Observability
- Spans: `agent.core.run`, `agent.sdk.tool`, `agent.policy.check`, `agent.budget.check`.
- Structured logs; redaction of sensitive fields; metrics for latency and errors.
