# Cost Management Guide (Phase 3)

## Overview
Budgets protect runs from unbounded spend by enforcing limits on tokens and cost. ORCA tracks usage per run and per agent, emits warnings at thresholds, and halts deterministically when budgets are exceeded.

## Configure Budgets

- Per-run budget (preferred): set on StartRun

```proto
message Budget {
  uint64 max_tokens = 1;        // 0 = unset
  uint64 max_cost_micros = 2;   // 0 = unset
}
```

- Environment defaults (applies when StartRun.budget is unset):
  - `ORCA_MAX_TOKENS`
  - `ORCA_MAX_COST_MICROS`

## Usage Tracking

- Counters recorded per run and per agent (tokens, cost_micros)
- Events:
  - `usage_update` (running totals)
  - `run_summary` (final totals + per-agent breakdown)
- Warnings:
  - `budget_warning` (levels: 80, 90)
- Exceeded:
  - `budget_exceeded` (run halts; subsequent tasks rejected with RESOURCE_EXHAUSTED)

## Telemetry

- Internal counters: aggregated in-process
- OTel metrics (names reserved; export behind feature flag):
  - `orca.tokens.total`
  - `orca.cost.micros.total`

## SDK Usage

- Per-run budget (recommended):
  - Python: set `StartRunRequest(budget=Budget(max_tokens=..., max_cost_micros=...))`
  - TypeScript: include `{ budget: { max_tokens, max_cost_micros } }` in `StartRun`
- Handling errors:
  - Check for `RESOURCE_EXHAUSTED` and surface a budget exceeded message to the user

## Examples

- Python (excerpt):
```python
start = pb.StartRunRequest(workflow_id="wf-1", initial_task=env, budget=pb.Budget(max_tokens=1000))
try:
    await stub.StartRun(start, metadata=md)
except grpc.aio.AioRpcError as e:
    if e.code() == grpc.StatusCode.RESOURCE_EXHAUSTED:
        print("Budget exceeded:", e.details())
```

- TypeScript (excerpt):
```ts
client.StartRun({ workflow_id: 'wf-1', initial_task: env, budget: { max_tokens: 1000 } }, md, (err) => {
  if (err?.code === grpc.status.RESOURCE_EXHAUSTED) console.error('Budget exceeded:', err.details)
})
```

## Troubleshooting

- I expected a warning or exceed and didn’t get one:
  - Verify StartRun budget and/or env defaults were set
  - Confirm SDK is providing usage hints where available
- I got RESOURCE_EXHAUSTED too early:
  - Check counters in `usage_update` events; confirm correct usage hints
- Logs too noisy:
  - Use `StreamEvents.max_events` or filter on event name

## Notes

- Redaction: budget events must contain no sensitive payloads; rely on policy hooks for redaction
- Determinism: exceed decisions are deterministic based on counters and configured limits
