---
type: "always_apply"
---

Engineering Standards (ORCA, Rust-first, Safety-Critical)

Purpose
- Replace verbose, language-specific guidance with concise, enforceable rules for ORCA.
- Preserve mission-critical posture: deterministic, fail-closed, auditable, observable, reproducible.
- Rust is the primary language; defer detailed idioms to rust-standards.md, testing-validation.md, performance-optimization.md, and security-privacy.md.

1) Agentic workflow (always-on)
- Pick and state a thought framework: CoT (debugging/logic), ToT (architecture/options), SCoT (perf/large refactors), First Principles (novel/ambiguous), ReAct (explore+act).
- Stratified context retrieval: Stratum 1 (locate files/modules/tests) → Stratum 2 (inspect key functions/traits) → Stratum 3 (precise change points/invariants). Stop when enough to proceed.
- Checkpoint gates (STOP): before cross-cutting or multi-file changes; before external/side-effects (network/DB/deploy/install); after test failures.
- Iterative loop: Reflection → Minimal change → Local validation → Keep/refine.
- Acceptance-criteria-first (TDD encouraged). Define tests up front; prefer RED → GREEN → REFACTOR.

2) Safety-critical baseline
- Fail-closed defaults; deny on error/misconfig. No ambient authority.
- Determinism required for core runtime: fixed seeds; stable ordering; no wall-clock dependencies on control paths; virtual clock in replay.
- Event-sourced state: all transitions recorded in WAL v2; replay must converge bit-for-bit within defined tolerances.
- Resource bounds: explicit time/CPU/memory/token limits; graceful backoff; bounded queues; no unbounded growth.
- Auditability: structured logs + immutable WAL; trace causality and RunId across boundaries.

3) Determinism & replay
- Single writer invariant per WAL stream; fsync at checkpoints; stable serialization (field order, float formatting, locale-free).
- External I/O capture via proxies/interceptors; record request/response digests and determinism metadata in WAL.
- Simulation mode: virtual time, deterministic randomness, recorded external effects; “replay” treats WAL as ground truth.

4) Concurrency policy
- Prefer message passing. If shared memory, specify thread-safety and memory ordering; avoid relaxed semantics on control paths.
- Validate with Loom (interleavings) and miri (UB); use sanitizers in CI when feasible.
- Safe reclamation where applicable; no data races; bounded parallelism.

5) Performance budgets (default unless task overrides)
- No >5% regressions (CPU, peak mem, latency) without explicit approval and evidence (95% CI).
- Orchestrator path: p95 per-call latency budget explicit in AC; target ≤10–15% overhead when capture/guardrails enabled.
- WASM plugins: cold start ≤200 ms p95; warmed invoke ≤5 ms p95; Blob IO ≥80 MB/s sustained.
- WAL overhead: record ≤3% p95; replay ≤1% p95.

6) Testing & CI gates
- Coverage: ≥85% overall; ≥90% for core/critical paths.
- Treat warnings as errors: clippy -D warnings; rustc deny(warnings) in CI for core crates.
- Include property tests and fuzzing for parsers/serializers; golden files for WAL/Envelope.
- Explicit validation commands (examples):
  - cargo test --workspace --all-features -- --nocapture
  - cargo clippy --workspace -D warnings
  - cargo fmt --all -- --check
  - RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -p <crate>  # optional ASan

7) Security & privacy (fail-closed)
- Secrets: never log; redact; store securely; rotate.
- Governance: tool allowlist first; classifier errors → deny; RBAC hooks; egress PII redaction.
- Isolation: WASI default; explicit capabilities; signed manifests (Sigstore/Cosign) and digest pinning for plugins.
- Networking: mTLS; header allowlist; timeouts and backoff; input validation at trust boundaries.

8) Observability (OTel-first)
- Spans around every external boundary; structured logs; metrics with clear units (ms, bytes, count).
- Required metrics (illustrative): wal.append.ms, wal.flush.ms, orch.clock.now.count, proxy.rtt.ms, llm.capture.ms, governance.decision.count{allow,deny,flag}, plugin.invoke.ms, blob.put/get.bytes.
- Trace attributes: run_id, wal_version, schema_migration, redaction_profile, model/provider.

9) API design (language-agnostic)
- Stable, versioned public APIs; explicit error models; no silent failures.
- Two-call buffer sizing for C-like APIs; explicit status codes; opaque handles; ABI stability across minor versions where applicable.
- SDKs (Py/TS): strict typing, retries with caps, idempotency keys where relevant; align with server semantics.

10) Config & flags
- Safe defaults (deny-by-default). Feature flags gate risky paths: wal_v2, use_virtual_clock, capture_enabled, bypass_to_direct.
- Document rollback/forward-compat plans for any new flag.

11) Review checklist (pre-merge)
- AC covered by tests; budgets met; observability added; gates wired in CI.
- STOP gates honored; rollback plan documented; security posture reviewed; determinism verified.
- Artifacts & repro: commands, seeds, env manifest, logs/traces, golden files.

References (authoritative detail)
- .augment/rules/rust-standards.md — language idioms, error handling, concurrency.
- .augment/rules/testing-validation.md — pyramid, coverage, CI gates, property/fuzz tests.
- .augment/rules/security-privacy.md — RBAC, secrets, redaction, sandboxing.
- .augment/rules/performance-optimization.md — profiling, async, memory, flame graphs.
- .augment/rules/agentic-architecture.md — deterministic replay, budgets, isolation, policies.

Notes
- This file intentionally replaces verbose examples/rationales with compact, actionable rules.
- Project-specific decisions/historical context should live in Docs/ or memories; do not bloat rules.
