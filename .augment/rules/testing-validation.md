---
description: Testing frameworks, validation patterns, and quality assurance standards
globs: ["tests/**", "**/*_test.*", "**/*test*", "bench/**"]
alwaysApply: false
---

# Rule: Testing & Validation

Acceptance gates (CI must enforce):
- Coverage: ≥85% overall; ≥90% for core/runtime crates; no drop on diff.
- Lint/format: zero warnings; clippy pedantic clean; rustfmt stable format.
- Determinism: fixed seeds; stable ordering; numeric tolerances documented (Roadmap Phase 0/Global).
- Performance: no >5% regressions in CPU/latency/peak mem without explicit approval + evidence.

CI matrix:
- macOS and Linux required from Phase 0; add Windows in Phase 7 before release sign-off.

Rust (core):
- Unit tests with `cargo test` (or nextest); property tests via `proptest`; fuzz via `cargo-fuzz`.
- Benchmarks via `criterion` with JSON/CSV outputs checked into `benchmarks/`.
- Static analysis: `cargo clippy -- -D warnings` (see `clippy.toml`); `cargo fmt -- --check`.
- Security: `cargo audit` and `cargo deny` in scheduled CI.

Python (SDK/tools):
- Lint: `ruff` (errors), format `black`, type-check `mypy --strict`.
- Tests: `pytest -q --maxfail=1 --durations=10` with coverage ≥85%.

TypeScript (SDK/tools):
- Lint: `eslint` with `@typescript-eslint` (errors); format via `prettier --check`.
- TS: `tsc --noEmit --pretty false` with `strict: true`.
- Tests: `vitest --run` or `jest --ci` with coverage ≥85%.

Sanitizers & extra checks (opt-in):
- Rust ASan/LSan/TSan on nightly for concurrency/memory hot spots.
