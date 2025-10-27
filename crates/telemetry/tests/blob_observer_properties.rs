#![cfg(feature = "otel")]

use blob_store::{set_observer, BlobStore, DevKeyProvider};
use proptest::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use telemetry::blob_observer::{global as blob_global, snapshot_counters};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn unique_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let p = base.join(format!("orca_blob_obs_prop_{}_{}", std::process::id(), id));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

proptest! {
    #[test]
    fn metrics_increment_across_sizes(sz in prop_oneof![
        Just(0usize),
        Just(1usize),
        Just(1024usize),
        0usize..=4096usize,
        Just(1024 * 1024usize)
    ]) {
        // Register the OTel-backed observer (idempotent)
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { set_observer(blob_global()); });

        let before = snapshot_counters();

        let dir = unique_dir();
        let cfg = blob_store::Config { root: dir.clone(), zstd_level: 3 };
        let store: BlobStore<DevKeyProvider> = BlobStore::new(cfg, DevKeyProvider::new([7u8; 32])).unwrap();

        let data = vec![7u8; sz];
        let dg = store.put(&data).unwrap();
        let got = store.get(&dg).unwrap();
        prop_assert_eq!(got, data);

        let after = snapshot_counters();
        let put_delta = after.0.saturating_sub(before.0);
        let get_delta = after.1.saturating_sub(before.1);

        prop_assert!(put_delta >= sz as u64);
        prop_assert!(get_delta >= sz as u64);

        let _ = fs::remove_dir_all(&dir);
    }
}
