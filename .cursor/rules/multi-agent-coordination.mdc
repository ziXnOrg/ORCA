---
description: Multi‑Agent Coordination (Vesper) — roles, comms, state, errors; refs to observability/testing/perf/security
alwaysApply: true
---

# Multi‑Agent Coordination (Authoritative)

Authoritative for: roles/responsibilities/invariants, memory‑based comms, state transitions, error/recovery policies.
References: `agentic-architecture.mdc` (concurrency, WAL/snapshot, determinism), `observability.mdc`, `testing-validation.mdc`, `performance-optimization.mdc`, `security-privacy.mdc`.

## Roles & Responsibilities (MUST)
- Orchestrator: own task DAG; assign; enforce guardrails (timeouts, backpressure, idempotency); initiate consensus (vote/debate/avg). Forbidden: bypassing gates; unbounded tooling.
- CodeGenAgent: produce minimal diffs + rationale; deterministic formatting; no merges.
- TestAgent: Gen (tests+rationale, seeded); Run (hermetic run, structured report). No prod code writes.
- StaticAnalysisAgent: issues with severity/locations/rule IDs; optional patch.
- DebugAgent: root‑cause + candidate patch; cite evidence.
- PerformanceAgent: profiles, quantified deltas, hardware/config recorded.
- DesignReviewAgent: architectural findings/risks referencing standards.

## Communication Protocols (MUST)
- Memory‑based routing in Vesper with roaring bitmap filters.
- JSON Envelope (required): id, parentId|null, role, taskId, status[proposed|in_progress|completed|blocked|failed|cancelled], priority, requires_reply, timestamp(RFC3339), version.
- TTL: interim=min/hours; final=days/weeks; all updates versioned.
- Observability: MUST comply with `observability.mdc` for span naming/attrs and log policy.

## Workflow Patterns (SHOULD)
- TDAG decomposition to leaf size/clarity thresholds.
- Assignment via skill library + reputation + diversity threshold (engage >1 agent if top scores within ε).
- Consensus: weighted voting; debate; average consensus for continuous values.
- Overhead budget: coordination+retrieval <10% total task time (see `performance-optimization.mdc`).

## State & Consistency (MUST)
- States: proposed → in_progress → {completed|blocked|failed|cancelled}.
- Transitions require evidence (tests/logs/metrics). Record idempotency keys for merges/external ops.
- Concurrency: single‑writer orchestrator; RCU readers for agents (see `agentic-architecture.mdc`).
- Snapshots & WAL: checkpoint at safe boundaries; rollback restores snapshot then re‑enqueue idempotent tasks.

## Error Handling & Recovery (MUST)
- Taxonomy: invalid_argument, not_found, unavailable, resource_exhausted, cancelled, io_failed, internal.
- Retries: transient only (unavailable|resource_exhausted|io_failed) with bounded exponential backoff + jitter.
- Circuit breakers: open on repeated transient failures; fallback/alternate flows; annotate decisions.
- Deadlines/cancellation required on RPC/tool calls; record root cause.
- Recovery: replay WAL to last snapshot; reissue idempotent tasks only.

## Quality & Security (MUST)
- Testing & gates: comply with `testing-validation.mdc` (coverage, sanitizers, evidence).
- Performance: comply with `performance-optimization.mdc` (SLOs, overhead budget).
- Security & privacy: comply with `security-privacy.mdc` (TLS/mTLS, RBAC, redaction, audit events).

