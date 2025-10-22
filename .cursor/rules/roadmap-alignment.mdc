---
description: Roadmap alignment — phases, gates, and traceability to rules
globs: ["Docs/Roadmap.md", ".cursor/rules/**"]
alwaysApply: true
---

# Rule: Roadmap Alignment (Authoritative)

Source of truth:
- `Docs/Roadmap.md` defines phased deliverables (Phase 0–7) and global success criteria. Rules and CI must align.

Traceability:
- Maintain a mapping from Roadmap phases → rules and acceptance gates (see Testing & Validation and Performance Optimization).
- For each phase completion, append evidence to `Docs/dev_log.md` (commands, metrics, seed, artifacts) and reference related rules.

Gates:
- Phase 0: event log prototype, basic OTel, security stubs, CI/tooling ready.
- Phase 1: deterministic orchestrator, message schema, contracts, WAL integration.
- Phase 2: Python/TS SDK parity and unified metadata model.
- Phase 3: budget manager and enforcement telemetry.
- Phase 4: full tracing coverage, sampling, replay tool, debug guide.
- Phase 5: policy engine (content/tool), redaction, audit events.
- Phase 6: auth/RBAC, tenant isolation, TLS, privacy controls.
- Phase 7: Windows support, perf polish, integrations, release artifacts.

Global criteria:
- Coverage ≥85% (≥90% core); determinism (fixed seeds); low-cardinality telemetry; orchestration overhead <10%; auditability and security posture.