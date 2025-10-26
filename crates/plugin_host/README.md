# plugin_host crate

This crate hosts the Wasmtime-based plugin runtime and plugin manifest verification.

- Manifest verification is fail-closed and deterministic (no network)
- Sigstore bundle verification is being integrated as a follow-up to T-6a-E3-SEC-04
- See the operational signing guide:
  - Docs/plugin_signing_runbook.md — how to sign plugins and how ORCA verifies them offline

Status
- Current: hardened digest validation and signature size caps (PR #63)
- Next: offline Sigstore bundle verification with pinned trust roots (Milestone M2)

