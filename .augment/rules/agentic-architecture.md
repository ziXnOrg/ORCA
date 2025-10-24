---
type: "agent_requested"
description: "Agentic AI system architecture patterns and cognitive frameworks"
---

# Rule: Agentic Architecture & Coordination

Authoritative guidance for ORCA runtime architecture, aligned to `Docs/Blueprint.md`.

Core invariants:
- Event-sourced execution: every external input/output becomes an immutable event (WAL-first).
- Deterministic replay: re-execute runs by substituting recorded outputs; stable seeds; time abstraction.
- Budgets: enforce per-run cost/token/time; pre-check before expensive actions; fallbacks allowed by policy.
- Isolation: each agent runs in a sandboxed process/container with least privilege; Core mediates I/O.
- Policy engine: allow/deny/modify/flag decisions on prompts/responses/tools; hot-reloadable rules.

State & consistency:
- Single-writer orchestrator for control-plane mutations; readers via snapshot/RCU.
- WAL + atomic apply; crash-safe recovery to last durable event.
- Idempotent handlers and idempotency keys for external integrations.

Execution gates:
- Timeouts and cancellations are mandatory for all tool/LLM calls.
- Retries: transient only (bounded backoff + jitter). Circuit-breakers on repeated failures.

ABI & API:
- Stable gRPC/IPC protocol for SDKs. Version and document envelopes.
- Structured JSON/event envelopes with low-cardinality attributes for observability.

Determinism:
- Fixed seeds; ordered reductions; no wall-clock dependence in logic; record nondeterministic values.

Security hooks:
- RBAC check-points at run start, tool invocation, and data egress; opt-in human-approval policies.