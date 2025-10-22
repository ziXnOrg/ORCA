---
alwaysApply: true
---

# Rule Name: vesper-workflow
Description: Always-applied seven-phase workflow for the Vesper Agentic AI project.

## Seven-phase framework (apply strictly; 1→7)

1) Research and discovery
   - Read: `docs/roadmap.md`, `docs/code-quality-standards`, agentic framework docs, `.cursor/rules/*`, affected headers/sources/tests.
   - Review: WAL invariants (manifest/snapshot/purge), HNSW concurrency/filtered search, SIMD dispatch/parity notes, KMeans tests and inertia checks, performance budgets.
   - Optional: targeted web/API checks for platform specifics (cite when used).
   - Define scope, dependencies, alternatives, and trade-offs; list assumptions and risks.
   - Enforce:
     - Add/append `docs/dev_log.md` entry with Findings/Scope/Assumptions/Risks.
     - Create/merge TODOs for this effort (atomic, outcome-oriented).

2) Plan and design (from first principles)
   - Choose: data layout (SoA), synchronization (single-writer + RCU readers), runtime dispatch (Accelerate/AVX/NEON/scalar), naming (snake_case/CamelCase), API/ABI boundary.
   - Define: minimal public C-ABI and internal C++ interfaces; explicit error via `std::expected`; determinism handling strategy; acceptance gates and rollback criteria.
   - Output: step-by-step plan with file/function touch list and test/perf gates.
   - Enforce:
     - Update `docs/dev_log.md` with Plan/Acceptance.
     - Split plan into TODOs (one in_progress; others pending).

3) Implementation
   - Implement narrowly scoped edits; keep ABI stable; RAII; explicit nullptr checks; named `constexpr`s; no drive-by refactors.
   - Determinism: ordered reductions or scalar fallback where required; avoid virtuals in hot paths; 64B alignment; avoid accidental O(N^2).
   - SIMD: provide scalar fallback; guard feature checks; prefer Accelerate on Apple.
   - Documentation: add concise “why” and Doxygen headers on changed public files.
   - Enforce:
     - clang-format + clang-tidy clean before commit (warnings-as-errors profile).
     - Update `docs/dev_log.md` with Edits/Rationale.

4) Staff review (self-critical enrichment)
   - Assess: clarity, correctness, determinism, perf complexity, naming/cohesion, API stability, testability.
   - Identify enrichments: cache locality, coalesced reads, lock-free opportunities, memory reuse, telemetry coverage.
   - Enforce:
     - Add Review notes + concrete Actions to `docs/dev_log.md`.
     - Create TODOs for follow-ups; keep one in_progress at a time.

5) Improve based on review
   - Apply targeted refinements while preserving readability and ABI stability.
   - Keep diffs minimal; avoid unrelated reformatting; retain determinism guards.
   - Enforce:
     - Re-run linters; update `docs/dev_log.md` with Improvements.
     - Conventional commits with objective and scope.

6) Tests (smoke, unit, integration, perf)
   - Write tests for intended behavior; include:
     - WAL invariants: manifest/snapshot/purge; replay delivery limits; fsync profiles.
     - HNSW: FilteredSearch correctness; MemoryManagement safety; concurrency invariants.
     - SIMD: backend parity vs scalar; numeric tolerances; accumulators where hot.
     - KMeans: inertia alignment; reproducible seeds; perf micro-bench JSON/CSV.
   - Ensure tests are hermetic, deterministic (fixed seeds), and fast.
   - Enforce:
     - Run `ctest -R "wal|hnsw|kmeans|simd" --output-on-failure` locally.
     - Sanitizers (ASan/UBSan/TSan) on demand for new/changed areas.
    - Attach evidence (test names, logs, perf CSV/JSON) in `docs/dev_log.md`.
    - Multi-agent overlay: follow `multi-agent-coordination.mdc` for roles/comms/state/errors; use `observability.mdc` for spans/logs; do not restate numeric gates here.

7) Standards and CI integration
   - Ensure format/tidy clean. Numeric gates and SLOs are authoritative in `testing-validation.mdc` and `performance-optimization.mdc`.
   - Update telemetry/log schemas and docs (README/ROADMAP) when behavior changes; ensure artifacts and `.gitignore` correct.
   - Cross-platform check: Windows/Linux/macOS builds/tests for impacted modules.
   - Enforce:
     - Block merge if gates fail; record Validation (commands/results) in `docs/dev_log.md`.
     - Keep PRs small; include objective, constraints, acceptance, risks, evidence, rollout notes.