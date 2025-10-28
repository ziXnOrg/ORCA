# ORCA Policy Engine (Governance Baseline)

Purpose
- Enforce governance decisions over envelopes and task submissions with a fail‑closed security model.
- Deterministic evaluation: priority → most‑restrictive → first‑match.
- Deny on error/misconfiguration by default (no ambient allow).

Key Features
- Decision taxonomy: Allow | Deny | Modify (e.g., PII redaction)
- Tool allowlist enforcement
- Observability: low‑cardinality counters per decision
- Audit: per‑decision AuditRecord capture for testability

Quick Start (observability)
```rust
use orca_policy as policy; // crate name alias used by the workspace

// Install an in‑process observer (non‑blocking, cheap)
struct Capture;
impl policy::PolicyObserver for Capture {
    fn on_decision(&self, phase: &str, d: &policy::Decision) {
        eprintln!("policy decision: {phase} → {:?}", d.kind);
    }
}
policy::set_observer(Some(Box::new(Capture)));

// Metrics: low‑cardinality counters keyed by {phase, kind, action}
let m = policy::policy_metrics();
let before = m.decision_counter("pre_submit_task", "deny", "deny");
// ... run an evaluation via the orchestrator/policy engine ...
let after = m.decision_counter("pre_submit_task", "deny", "deny");
assert!(after >= before);

// Audit sink: capture per‑decision records (helpful in tests)
let sink = policy::install_audit_sink();
let records = sink.drain();
assert!(records.iter().all(|r| !r.phase.is_empty()));

// When done, clear the observer
policy::set_observer(None);
```

Notes
- The special action `allow_but_flag` also increments a `flag` alias for convenience
  when querying metrics.
- All hooks are designed to be low‑overhead and test‑friendly; avoid blocking I/O in observers.
- See crate rustdocs for precedence rules and full API details.

