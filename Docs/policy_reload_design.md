# Policy Reload Design (Phase 5 Task 2)

## Current Implementation Overview
- Policy engine instance is owned by OrchestratorService inside `Arc<RwLock<policy::Engine>>`.
- Initial load: if `ORCA_POLICY_PATH` is set, the engine loads the YAML at service init.
- Optional hot-reload: if `ORCA_POLICY_RELOAD_MS` is a positive integer, a background Tokio task periodically acquires a write lock and re-loads from the same path.
- Enforcement is read-only from request handlers via `policy.read().unwrap()` ensuring concurrent reads while no write is in progress.

## Thread-Safety and Memory Ordering
- Synchronization primitive: `std::sync::RwLock` (poison-aware, blocking). Reads and writes are mutually exclusive, guaranteeing a consistent policy view.
- Memory ordering: RwLock establishes happens-before edges between writers and subsequent readers; a reader always sees a fully-initialized policy snapshot after a successful write lock scope exits.
- Engine fields are owned data (no interior mutability except through `RwLock`), avoiding data races.

## Atomicity and In-Flight Requests
- Atomicity granularity: reload is atomic at the engine snapshot level. Either the old engine state is used (during reload) or the new one (after write lock release).
- In-flight requests: handlers take a read lock on entry. Requests that begin before the write lock is acquired use the old policy; requests that begin after the write lock completes use the new policy.
- No partial states are observable by handlers. Failure to load keeps the previous valid snapshot (fail-closed for errors that occur during enforcement remains enforced by rules/allowlist).

## Error Handling Semantics
- Load errors (IO or YAML/validation):
  - Do not swap in a new engine state.
  - Log an error and retain the previous policy (best-effort continuity, no relaxation of existing deny rules).
- Validation errors: YAML must pass structural validation before activation (actions, allowlist quality, transforms).
- Enforcement-time errors: engine methods are pure and do not error; deny-by-default is enforced via allowlist checks and rule precedence (see policy module docs).

## Precedence Model (Rules)
- Highest priority wins (larger integer = higher priority).
- Equal priority tie-break: Most-restrictive-wins (Deny > Modify > Allow). If still tied (same restrictiveness), first-match-wins according to file order.
- Built-ins precedence: Built-in PII redaction runs before allowlist; allowlist enforcement runs before rule evaluation (security-first fail-closed).

## Future Admin API (Not Implemented Here)
- Surface: gRPC method `ReloadPolicy(PolicySource)` or a management-only endpoint in the same service.
- Authentication/Authorization: mTLS with client certs; role-bound (e.g., `policy.admin`); enforce via middleware (Casbin or static ACLs) and audit all reload attempts.
- Source options:
  - File path (re-parse from disk)
  - Inline YAML payload (prefer staged validation without activation on error)
  - Versioned store reference (e.g., Git SHA, object store URL)
- Behavior:
  - Validate-only mode: parse+validate without activation.
  - Activate-on-success: parse+validate+swap under write lock.
  - Emit `policy_audit` admin events for reload attempts (success/failure) with requester identity.

## Security Considerations
- Strict input validation: allowed actions (deny|modify|allow_but_flag); allowlist sanitized, deduped, and case-folded.
- Do not log secrets/PII in reload paths; audit only metadata (source, result, rule counts).
- Principle of least privilege: restrict who can trigger reloads; rate-limit and monitor.
- Defense-in-depth: built-in deny-by-default allowlist and precedence ensure that absent policy cannot weaken enforcement.

## Testing Strategy
- Unit: policy YAML validation (malformed YAML, invalid actions, regex transform errors, allowlist quality), rule precedence (priority and tie-breaks), redaction.
- Integration: orchestrator pre/post hooks continue to enforce and emit audits before/after reload; ensure deterministic behavior with concurrent reads while a reload occurs.
- Concurrency tests: stress test reads while reloading (use multiple tasks); assert no panics, no partial reads, and consistent decisions.
- Failure injection: simulate reload parse failures; verify old policy remains active and failures are logged/audited.

## Compatibility & Migration
- Priority field defaults to 0 (backward compatible with existing `policy.yaml`).
- Existing rules without `priority` retain original behavior (file order only). Equal priority now resolves via most-restrictive, which is a security improvement.
- Operators can gradually introduce `priority` to refine conflict resolution.

