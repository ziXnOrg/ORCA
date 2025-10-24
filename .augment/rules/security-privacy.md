---
description: Security, privacy, and cryptographic implementation standards
globs: ["**/*security*", "**/*crypto*", "**/*privacy*", "**/*encrypt*"]
alwaysApply: false
---

# Rule: Security & Privacy

Threat model & posture:
- Default deny. Least privilege for sandboxes and tools. No network/filesystem by default.
- Enforce RBAC checkpoints for agent start, tool invocation, and data egress.

Secrets & keys:
- Never hard-code secrets. Load via env/secure store. Redact in logs. Zeroize in memory when possible.
- Prefer Core-side proxying for third-party APIs so sandboxes don’t receive provider secrets.

Input/Output validation:
- Validate sizes, formats, and bounds for all external inputs. Fail closed on invalid data.
- Content moderation hooks on prompts/responses; redact sensitive fields at policy layer.

Logging/Audit:
- Structured logs only; no raw content in exported telemetry. Support tamper-evident chaining for audit logs.
- Tenant isolation: tag all data by tenant; forbid cross-tenant access by policy and storage partitioning.

Crypto:
- TLS for all SDK↔Core comms. Encrypt persisted logs at rest. Consider AEAD for sensitive fields.

Recovery:
- WAL-driven recovery to consistent state; forensic mode captures minimal extra diagnostics (no secrets).
