# ADR 0001: Transport for Orchestrator Service

## Status
Accepted

## Context
ORCA SDKs (Python, TypeScript) must communicate with a Rust orchestrator service to submit tasks, stream events, and fetch results with low overhead and strong typing. We require:
- Bi-directional or server streaming for event streams
- Strongly typed contracts and versioning
- Cross-language client support
- Compatibility with observability (tracing) and mTLS

## Options Considered
- HTTP/JSON (REST via axum/actix)
  - Pros: ubiquitous tooling, easy to debug, minimal client frictions
  - Cons: no native streaming contracts beyond chunked/WebSocket; looser typing; more boilerplate for envelopes
- GraphQL (async-graphql)
  - Pros: flexible querying; schema-first
  - Cons: not a natural fit for ordered event streaming; added complexity vs need
- gRPC over HTTP/2 (tonic)
  - Pros: IDL-driven, codegen for Python/TS clients, native streaming RPCs, efficient
  - Cons: steeper setup; binary payloads make ad-hoc debugging harder

## Decision
Adopt gRPC over HTTP/2 using `tonic` as the primary transport.
- Package names and service definitions include versioning, e.g., `orca.v1`.
- Use server streaming for event playback (`StreamEvents`) and unary for control (`StartRun`, `SubmitTask`).
- Payloads carry JSON (envelope payload) as UTF-8 strings initially for flexibility; later phases can define typed messages.
- Admin/ops endpoints MAY expose minimal HTTP/JSON in a separate port in later phases (not in-scope here).

## Consequences
- SDKs: generate clients for Python/TS against the proto; ensure consistent envelope fields and `protocol_version`.
- Observability: propagate OpenTelemetry via interceptors; map span attributes; low-cardinality.
- Security: enable TLS/mTLS when configured; tokens carried via metadata headers; RBAC checks at endpoints.
- Versioning: breaking changes require new `orca.v2` package; deprecate older packages per policy.
