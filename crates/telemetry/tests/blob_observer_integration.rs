#![cfg(feature = "otel")]

use blob_store::{set_observer, BlobStore, DevKeyProvider};
use std::fs;
use std::path::PathBuf;
use telemetry::blob_observer::{global as blob_global, snapshot_counters};

fn temp_dir_path() -> PathBuf {
    let base = std::env::temp_dir();
    let p = base.join(format!("orca_blob_obs_{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn registers_observer_and_counts_metrics() -> Result<(), Box<dyn std::error::Error>> {
    // Register the OTel-backed observer
    let _ = set_observer(blob_global());

    // Create a store and exercise put/get/cleanup
    let dir = temp_dir_path();
    let cfg = blob_store::Config { root: PathBuf::from(&dir), zstd_level: 3 };
    let store: BlobStore<DevKeyProvider> = BlobStore::new(cfg, DevKeyProvider::new([9u8; 32]))?;

    let data = b"abc".to_vec();
    let dg = store.put(&data)?;
    let got = store.get(&dg)?;
    assert_eq!(got, data);

    // Create an incomplete artifact to trigger cleanup
    let shard = store.path_for(&dg.to_hex());
    let tmp = shard.with_extension("incomplete");
    fs::create_dir_all(tmp.parent().unwrap())?;
    fs::write(&tmp, b"junk")?;
    let removed = store.cleanup_incomplete()?;
    assert!(removed >= 1);

    let (put_bytes, get_bytes, clean) = snapshot_counters();
    assert!(put_bytes >= data.len() as u64);
    assert!(get_bytes >= data.len() as u64);
    assert!(clean >= 1);

    // Cleanup
    let _ = fs::remove_dir_all(&dir);

    Ok(())
}
