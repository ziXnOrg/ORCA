use std::path::PathBuf;
use std::time::Duration;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init_json_logging();

    #[cfg(feature = "otel")]
    {
        telemetry::init_otlp_from_env()?;
        blob_store::set_observer(telemetry::blob_observer::global());
    }

    // Temp directory for demo
    let dir = std::env::temp_dir().join("orca_blob_otlp_demo");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;

    // Create a blob store
    let cfg = blob_store::Config { root: PathBuf::from(&dir), zstd_level: 3 };
    let store: blob_store::BlobStore<blob_store::DevKeyProvider> =
        blob_store::BlobStore::new(cfg, blob_store::DevKeyProvider::new([0xAA; 32]))?;

    // Perform put/get to generate metrics and spans
    let data = b"hello otlp".to_vec();
    let dg = store.put(&data)?;
    let got = store.get(&dg)?;
    assert_eq!(got, data);

    // Create an incomplete artifact and cleanup to exercise cleanup metric
    let shard = store.path_for(&dg.to_hex());
    let tmp = shard.with_extension("incomplete");
    std::fs::create_dir_all(tmp.parent().unwrap())?;
    std::fs::write(&tmp, b"junk")?;
    let _removed = store.cleanup_incomplete()?;

    // Allow background exporters to flush
    #[cfg(feature = "otel")]
    {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    println!(
        "Blob OTLP demo completed. Endpoint: {}",
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4318".into())
    );

    Ok(())
}
