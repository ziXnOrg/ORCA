# Contributing

Thank you for contributing to ORCA.

## Standards & Gates

- Format: `cargo fmt --all -- --check`
- Lints: `cargo clippy --all-targets --all-features -- -D warnings`
- Tests: `cargo test --all --all-features`
- Coverage: CI publishes lcov (see Actions)
- Determinism: fixed seeds, stable ordering

## Pre-commit

Install hooks:

```
pre-commit install
```

## Security

- Do not commit secrets; pre-commit runs gitleaks.
- Dependencies audited via `cargo-audit`/`cargo-deny` (scheduled).

## Workflow

- Follow `Docs/Roadmap.md` phases and `.cursor/rules/` for changes.
- Update `Docs/dev_log.md` after material changes.
