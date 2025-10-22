# Jaeger Integration (Phase 4)

## Overview
This guide shows how to view ORCA traces in Jaeger using OpenTelemetry.

## Quick Start (Docker)

- Start Jaeger all-in-one:
```bash
docker run -d --name jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  -p 16686:16686 -p 4317:4317 -p 4318:4318 \
  jaegertracing/all-in-one:1.57
```
- Open Jaeger UI at `http://localhost:16686`

## Environment

- Set env to enable OTLP HTTP exporter (metrics/trace as needed):
```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_TIMEOUT=10000
```

- For traces via Jaeger exporter (optional alternative):
```bash
export OTEL_EXPORTER_JAEGER_ENDPOINT=http://localhost:14268/api/traces
```

## Metrics (OTLP)
- Metrics export is enabled under the `otel` feature. Configure via env:
```bash
export OTEL_EXPORTER_OTLP_METRICS_ENDPOINT=http://localhost:4318/v1/metrics
export OTEL_METRICS_EXPORT_INTERVAL=5000
export OTEL_SERVICE_NAME=orchestrator
```
- Verify using collector logs or your backend; counters:
  - `orca.tokens.total`, `orca.cost.total_micros`
  - histograms: `orca.tokens.per_task`, `orca.cost.per_task_micros`

## Run ORCA

- Build with `otel` feature enabled on crates that emit traces/metrics:
```bash
cargo build -p orchestrator --features otel
```

- In dev, confirm traces appear in Jaeger and metrics arrive at the collector.

## Notes

- Keep span attributes low-cardinality (see `Docs/.cursor/rules/observability.mdc`).
- Prefer OTLP → Collector → Jaeger pipeline for production.
- Use sampling config via env if high-volume: `OTEL_TRACES_SAMPLER=parentbased_traceidratio`, `OTEL_TRACES_SAMPLER_ARG=0.1`.
