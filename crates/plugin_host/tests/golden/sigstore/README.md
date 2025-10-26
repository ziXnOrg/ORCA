# Sigstore Bundle Test Fixtures (Offline)

Purpose
- Provide deterministic, offline-only fixtures for cosign-style Sigstore verification.
- No network access; pinned trust roots/bundles will be embedded as files.

Layout
- `valid_bundle.json` — placeholder stub (to be replaced with a real bundle)

Policy
- Tests using these fixtures must not perform network I/O.
- Verification remains fail-closed until real fixtures and verification logic are added.

