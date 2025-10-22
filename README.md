# ORCA (AgentMesh Runtime)

Rust-first orchestrator for deterministic, observable agent execution.

## Docs
- Architecture: Docs/Architecture.md
- Roadmap: Docs/Roadmap.md
- API proto: Docs/API/orca_v1.proto
- API how-to: Docs/API/HOWTO.md
- Cost management: Docs/cost_management.md
- Observability (Jaeger): Docs/observability.jaeger.md
- SDK Quickstarts: Docs/SDK/QUICKSTART_PY.md, Docs/SDK/QUICKSTART_TS.md
- Perf: Docs/perf.md
- Security (mTLS/RBAC): Docs/security.mtls.md, Docs/rbac.casbin.conf, Docs/rbac.policy.csv

## Build
```bash
cargo build --workspace
```

## Run (dev)
- See mTLS setup in Docs/security.mtls.md
- Start the orchestrator binary; then use SDK quickstarts to call the service.

## CI
See .github/workflows/ci.yml; proto artifact is uploaded for SDKs.
