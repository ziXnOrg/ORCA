---
description: High-performance Rust patterns and optimization strategies for ORCA runtime
globs: ["**/*.rs", "crates/**", "**/*simd*", "**/*perf*"]
alwaysApply: false
---

# Rule: Performance Optimization

Principles:
- Measure first. Benchmarks (Criterion) drive changes; guard P50/P99 latencies and memory.
- Data locality first: avoid unnecessary allocations; favor SoA layouts and contiguous storage.
- Orchestration overhead SLO: target <10% vs direct API calls (Roadmap global criteria).

Rust core:
- Zero-copy APIs where possible (slices, Cow, &str). Avoid cloning; use `#[inline]` for tiny hot functions.
- Async where I/O bound; sync where CPU bound. Avoid blocking in async contexts.
- Rayon or scoped threads for CPU parallelism; avoid oversubscription.
- Feature gates for optional heavy deps; minimal default features.

Budgets & backpressure:
- Enforce cost/time budgets early; preflight deny expensive steps exceeding remaining budget.
- Apply bounded queues; reject or shed load when capacity is saturated.

Observability cost:
- Batch/async log emission; redact/abbreviate large payloads. Configurable verbosity.

Tooling:
- `-C opt-level=3`, thin LTO in release; `codegen-units=1`. Perf CI tracks regression thresholds.
