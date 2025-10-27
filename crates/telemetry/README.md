# Telemetry (OTel) Integration for ORCA

This crate provides optional OpenTelemetry (OTel) wiring for metrics and tracing used across ORCA.

The Blob Store integrates with OTel via a pluggable observer. When the `otel` feature is enabled for this
crate, you can register an OTel-backed observer that emits counters and spans for blob operations.

## Features

- `otel` (optional): enables OTel SDK and a blob observer implementation
  - Metrics (low-cardinality):
    - `blob.put.bytes` (u64, By)
    - `blob.get.bytes` (u64, By)
    - `blob.cleanup.count` (u64)
  - Tracing: RAII spans around put/get/cleanup (best-effort)

## Environment variables (OTLP over HTTP)

- `OTEL_EXPORTER_OTLP_ENDPOINT` (e.g., `http://localhost:4318`)
- `OTEL_SERVICE_NAME` (e.g., `orchestrator`)
- Optional: `OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf`

Metrics/exporter configuration is set up lazily when metrics are first used.

## Minimal example

```rust
// Cargo features: telemetry = { features = ["otel"] }
use blob_store::{BlobStore, Config, DevKeyProvider, set_observer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize JSON logs; optionally initialize tracing via telemetry::init_otel("orchestrator")
    telemetry::init_json_logging();

    #[cfg(feature = "otel")]
    {
        // Register the OTel-backed observer for Blob Store (metrics + spans)
        set_observer(telemetry::blob_observer::global());
    }

    // Use the Blob Store as usual
    let dir = std::env::temp_dir().join("orca_blob_demo");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;

    let cfg = Config { root: dir.clone(), zstd_level: 3 };
    let store = BlobStore::new(cfg, DevKeyProvider::new([0x11; 32]))?;

    let data = b"hello".to_vec();
    let dg = store.put(&data)?;
    let got = store.get(&dg)?;
    assert_eq!(got, data);

    Ok(())
}
```

## Testing locally

- `cargo test -p telemetry --all-features -- --nocapture`
- `cargo test --workspace --all-features -- --nocapture`

No exporter needs to be running to exercise counters; tests use internal snapshot mirrors to assert increments.

