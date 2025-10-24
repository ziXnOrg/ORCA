# Debugging Guide (Phase 4)

## Quick checklist
- Enable JSON logs: RUST_LOG=info
- Optional traces/metrics (OTLP): see `Docs/observability.jaeger.md`
- Use `orca-replay` to inspect or export run traces from WAL

## Tracing (optional)
- Build with features that enable tracing (otel where applicable). Use env:
```
export OTEL_SERVICE_NAME=orchestrator
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
```
- View spans in Jaeger and check attributes: low-cardinality only.

## WAL Replay CLI
- Inspect:
```
orca-replay inspect --wal /path/to/log.jsonl --run-id RUN
```
- Replay with filters:
```
orca-replay replay --wal /path/to/log.jsonl --run-id RUN --from 10 --to 200 --since-ts-ms 0 --max 100 --dry-run
```
- Export to trace JSON:
```
orca-replay to-trace --wal /path/to/log.jsonl --run-id RUN --out trace.json
```

## Metrics
- Tokens/cost metrics (if otel enabled):
  - counters: `orca.tokens.total`, `orca.cost.total_micros`
  - histograms: `orca.tokens.per_task`, `orca.cost.per_task_micros`

## Redaction & Policy
- PII redaction occurs via Policy Engine hooks (pre_start_run / pre_submit_task).
- Verify redaction via tests and by inspecting WAL: sensitive substrings should be `[REDACTED]`.

## Common issues
- TTL expired: orchestrator returns DEADLINE_EXCEEDED.
- Budget exceeded: RESOURCE_EXHAUSTED; see usage_update and run_summary events.
- Missing spans: verify span coverage test; ensure tracing subscriber installed.
