# Plugin Signing Runbook (Sigstore, Offline-First)

Purpose
- Define a secure, offline-first process for signing ORCA plugins (WASM) and verifying them with pinned trust roots.
- Threat model: untrusted plugin artifacts, supply-chain tampering, key misuse. Trust boundaries: ORCA hosts verify bundles using pinned trust; no network or ambient trust.

Deterministic & Fail‑Closed
- Offline verification only (no network calls). Any error ⇒ deny load.
- Deterministic checks: stable SHA-256 digest; constant-time compare; pinned trust; fixed policies.

Key Management
- Generate signing keys:
  - Option A (Operator-managed CA; fully offline): create an internal CA, issue short-lived leaf certs for signing identities.
  - Option B (Public Sigstore keyless): use Fulcio + Rekor online, then archive the resulting bundle for offline use at verify-time.
- Storage: store CA and leaf private keys in HSM or sealed vault with RBAC; restrict export; rotate regularly.
- Rotation: rotate leaf keys frequently; rotate CA on schedule (document overlap periods and bundle re-signing plan).

Trust Model (Pinned Roots)
- ORCA pins trust roots via ManualTrustRoot (embedded or file paths):
  - Fulcio root/intermediate CA cert(s) for X.509 chain validation
  - Rekor public keys (for transparency log inclusion proof, when present)
  - CTFE keys (for certificate transparency proof, when enabled)
- No remote trust updates at verify-time. Updates are a deliberate release action.

Signing Workflow
- Inputs: plugin.wasm, identity (issuer/SAN), signing material (key pair + cert), and cosign installed.
- Produce a Sigstore bundle JSON alongside the plugin. Two common flows:

1) Offline operator CA (recommended for fully air-gapped)
- Generate CA and leaf (illustrative; adapt to your PKI/HSM):
  - Generate CA key/cert (rca.pem/rca.key) and leaf key/csr; sign CSR → leaf.pem.
- Sign the plugin and emit a bundle (cosign):
  - cosign sign-blob --key leaf.key --cert leaf.pem \
      --bundle plugin.sigstore.bundle.json plugin.wasm
- Notes: This embeds the signature and certificate chain. If you operate a Rekor-like log offline, emit and embed inclusion proof as well.

2) Connected mode with public Sigstore (archive result for offline verify)
- Keyless sign to Fulcio and Rekor (requires network during signing):
  - cosign sign-blob --bundle plugin.sigstore.bundle.json plugin.wasm
- Archive the produced bundle and the exact Fulcio/CTFE/Rekor trust roots at signing time; ship these as the pinned trust set for verification.

Verification Workflow (performed by ORCA)
- Compute SHA-256 of plugin.wasm and compare to manifest.wasm_digest (hex, 64 chars) using constant-time compare.
- Parse manifest.signature as Sigstore bundle JSON.
- Offline verification using pinned trust roots (ManualTrustRoot):
  - Verify signature over the content/digest
  - Verify X.509 chain to a pinned Fulcio CA (or operator CA)
  - If Rekor proof present: verify inclusion proof against pinned Rekor keys
  - Apply policy: issuer allowlist and SAN allowlist must match
- Any failure ⇒ InvalidSignature (fail-closed). No network.

Bundle Format (high-level)
- JSON with fields like:
  - mediaType: e.g., application/vnd.dev.sigstore.bundle+json;version=0.1
  - content: DSSE or messageSignature payload, including signature bytes or envelope
  - verificationMaterial: certificate chain, Rekor log entry + inclusion proof, and optional timestamping
- ORCA expects manifest.signature to be a full bundle that is sufficient for offline verification with our pinned trust roots.

Identity & Policy
- Issuer allowlist: exact match (e.g., https://fulcio.example/)
- SAN allowlist: one of email/URI/DNS must match allowed values
- Configurable per deployment; defaults deny unless explicitly allowed.

Troubleshooting
- Digest mismatch: re-export plugin.wasm; ensure manifest.wasm_digest matches sha256sum(plugin.wasm).
- Invalid signature: re-check keys/certs correspond; ensure bundle matches the exact bytes of plugin.wasm.
- Missing trust root: ensure pinned CA/Rekor/CTFE files are present and configured.
- Chain validation failed: verify leaf cert validity, EKU/code-signing, and issuer chain.
- Rekor proof failed: confirm proof corresponds to the signature and pinned Rekor key set.

Security Considerations
- Offline-only verification (no network); deny on error.
- Pinned trust roots; deliberate updates; audit changes.
- Short-lived signing certs; least-privilege storage; rotate keys.
- Deterministic verification; stable ordering; constant-time compares.

Operator Checklist
- [ ] Keys/certs managed and rotated
- [ ] Pinned trust roots recorded and distributed
- [ ] cosign version pinned for reproducibility
- [ ] Bundles archived with provenance metadata
- [ ] ORCA policy allowlists configured (issuer/SAN)

