# Performance: Benches & Profiling

## Criterion

- Orchestrator bench: `cargo bench -p orchestrator`

## Flamegraph

- Install: `cargo install flamegraph`
- Run (debug symbols): `cargo flamegraph -p orchestrator --bench submit`
- Open `flamegraph.svg` and identify hotspots.

## PGO (outline)

- Instrument build: `RUSTFLAGS='-Cprofile-generate' cargo build --release -p orchestrator`
- Run representative workload to collect `.profraw` files.
- Merge profiles (llvm-profdata), then rebuild with `-Cprofile-use`.
