# Rule: Rust Standards (Core Language of ORCA)

Style & formatting:
- Edition 2021+. Enforce `rustfmt` (stable defaults) with workspace config. No warnings allowed in CI.
- Module layout: `crates/<name>/src`, public API in `lib.rs`, minimal `pub use` re-exports.

Linting:
- `cargo clippy -- -D warnings -W clippy::pedantic -W clippy::nursery`.
- Allow specific lints only with justification at item scope.

Error handling:
- Prefer `Result<T, E>` with a project error enum (and `thiserror`). No panics in library code.
- Use `anyhow` only in binaries/tests; never in library crates.

Docs & safety:
- `#![deny(unsafe_code)]` by default. If `unsafe` is required, isolate and document invariants.
- Public items must have rustdoc. Enable `missing_docs` lint as warn or deny in workspace.

APIs & types:
- Zero-cost abstractions; prefer `&[u8]`, `&str`, `Cow` for borrowed data. Avoid `String`/`Vec` in APIs unless necessary.
- Thread-safety explicit: `Send`/`Sync` bounds where required; no global mutable state.

Concurrency:
- Prefer message passing (channels) and ownership transfer. If shared state is needed, use `Arc` + locks with narrow scopes.
- Avoid blocking in async; use `tokio::spawn_blocking` for CPU tasks if async.

Testing & benches:
- Unit tests colocated; property tests with `proptest`; fuzz targets where parsing/external input exists.
- Benchmarks via Criterion; commit baseline JSON/CSV and fail CI on >5% regression without approval.

Build & profiles:
- Release: LTO=thin, `codegen-units=1`, `opt-level=3`. Debug with `debug-assertions` on.