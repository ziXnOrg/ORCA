// RED phase tests for streaming IO (will fail to compile until implemented)
#![cfg(test)]

use blob_store::{deterministic_bytes, BlobStore, Config, DevKeyProvider};
use std::io::{Cursor, Write};
use std::path::PathBuf;

fn new_store() -> (tempfile::TempDir, BlobStore<DevKeyProvider>) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config { root: PathBuf::from(dir.path()), zstd_level: 3 };
    let store: BlobStore<DevKeyProvider> =
        BlobStore::new(cfg, DevKeyProvider::new([7u8; 32])).unwrap();
    (dir, store)
}

#[test]
fn digest_equality_streaming_vs_buffer_small_sizes() {
    let (_dir, store) = new_store();
    for &len in &[0usize, 1, 1024, 4096] {
        let data = deterministic_bytes(len);
        let d_buf = store.put(&data).expect("put buffer");
        let d_stream = store.put_reader(Cursor::new(data.clone())).expect("put_reader");
        assert_eq!(d_buf.0, d_stream.0, "digest mismatch at len={}", len);
        let got = store.get(&d_stream).expect("get after put");
        assert_eq!(got, data);
    }
}

#[test]
fn get_to_writer_matches_get() {
    let (_dir, store) = new_store();
    let data = deterministic_bytes(8192);
    let d = store.put(&data).unwrap();
    let mut out = Vec::with_capacity(data.len());
    let n = store.get_to_writer(&d, &mut out).expect("get_to_writer");
    assert_eq!(n, data.len());
    assert_eq!(out, data);
}

#[test]
fn cleanup_incomplete_still_works() {
    let (dir, store) = new_store();
    // Create a fake incomplete artifact
    let digest = store.put(&deterministic_bytes(16)).unwrap();
    let shard = store.path_for(&digest.to_hex());
    let tmp = shard.with_extension("incomplete");
    std::fs::create_dir_all(tmp.parent().unwrap()).unwrap();
    std::fs::write(&tmp, b"junk").unwrap();
    let removed = store.cleanup_incomplete().unwrap();
    assert!(removed >= 1);
    assert!(!tmp.exists());
    drop(dir);
}
