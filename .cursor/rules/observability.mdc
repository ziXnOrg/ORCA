---
description: Observability policy — OpenTelemetry spans/attributes, sampling, structured logs, redaction
alwaysApply: true
---

# Rule: Observability (Tracing, Logs, Metrics)

Span naming:
- `agent.core.run`, `agent.sdk.llm`, `agent.sdk.tool`, `agent.policy.check`, `agent.budget.check`.

Attribute allowlist (low-cardinality):
- `agent.role`, `run.id`, `task.id`, `status`, `result.count`, `error.code`.
- Never attach raw prompts/outputs to spans exported outside secure storage; use hashes/refs.

Structured logs:
- JSON logs: `event`, `run.id`, `task.id`, `error.code`(optional), `ts`.
- Redaction hooks for sensitive fields; do not log secrets or PII.

Metrics:
- Counters/gauges/histograms for runs, errors, latency (P50/P95/P99), token/cost usage.

Sampling & limits:
- Head sampling as configured; attribute length limits; drop unknown attributes.

Determinism:
- Stable ids, seeded tests/benchmarks; reproducible traces.

