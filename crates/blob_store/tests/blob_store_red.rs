use blob_store::{deterministic_bytes, BlobStore, Config, DevKeyProvider};
use std::fs;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn store_at(path: &std::path::Path, key: [u8; 32]) -> BlobStore<DevKeyProvider> {
    let cfg = Config::with_root(path.to_path_buf());
    let kp = DevKeyProvider::new(key);
    BlobStore::new(cfg, kp).unwrap()
}

#[test]
fn cas_digest_identity_and_idempotent_put() -> Result<()> {
    let dir = temp_dir();
    let store = store_at(dir.path(), [7u8; 32]);
    let data = b"hello world".to_vec();

    let d1 = BlobStore::<DevKeyProvider>::digest_of(&data);
    let d2 = store.put(&data)?;
    assert_eq!(d1, d2);

    // idempotent: same digest on repeated put
    let d3 = store.put(&data)?;
    assert_eq!(d2, d3);

    Ok(())
}

#[test]
fn round_trip_integrity() -> Result<()> {
    let dir = temp_dir();
    let store = store_at(dir.path(), [1u8; 32]);
    let data = deterministic_bytes(128 * 1024);
    let digest = store.put(&data)?;
    let got = store.get(&digest)?;
    assert_eq!(got, data);
    Ok(())
}

#[test]
fn wrong_key_fails_to_decrypt() -> Result<()> {
    let dir = temp_dir();
    let store_ok = store_at(dir.path(), [2u8; 32]);
    let data = deterministic_bytes(32 * 1024);
    let digest = store_ok.put(&data)?;

    // Second store at same root but different key must fail to read
    let store_bad = store_at(dir.path(), [3u8; 32]);
    let err = store_bad.get(&digest).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("wrong key") || s.contains("crypto") || s.contains("integrity"));
    Ok(())
}

#[test]
fn tamper_detection() -> Result<()> {
    let dir = temp_dir();
    let store = store_at(dir.path(), [4u8; 32]);
    let data = deterministic_bytes(16 * 1024);
    let digest = store.put(&data)?;

    // Mutate one byte on disk
    let path = store.path_for(&digest.to_hex());
    let mut bytes = fs::read(&path)?;
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xAA;
    fs::write(&path, bytes)?;

    let err = store.get(&digest).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("integrity") || s.contains("crypto"));
    Ok(())
}

#[test]
fn partial_write_detection_and_cleanup() -> Result<()> {
    let dir = temp_dir();
    let store = store_at(dir.path(), [5u8; 32]);
    let data = b"abc".to_vec();
    let digest = store.put(&data)?;

    // Create an incomplete artifact next to the final blob
    let path = store.path_for(&digest.to_hex());
    let tmp = path.with_extension("incomplete");
    fs::create_dir_all(tmp.parent().unwrap())?;
    fs::write(&tmp, b"partial")?;

    let cleaned = store.cleanup_incomplete()?;
    assert!(cleaned >= 1);

    Ok(())
}

#[test]
fn deterministic_behavior_across_runs_and_hosts() -> Result<()> {
    // Same bytes + key produce identical digest consistently
    let dir1 = temp_dir();
    let dir2 = temp_dir();
    let key = [9u8; 32];
    let store1 = store_at(dir1.path(), key);
    let store2 = store_at(dir2.path(), key);

    let data = deterministic_bytes(64 * 1024);
    let d1 = store1.put(&data)?;
    let d2 = store2.put(&data)?;
    assert_eq!(d1, d2);

    let g1 = store1.get(&d1)?;
    let g2 = store2.get(&d2)?;
    assert_eq!(g1, g2);

    Ok(())
}

#[test]
fn empty_blob_round_trip() -> Result<()> {
    let dir = temp_dir();
    let store = store_at(dir.path(), [6u8; 32]);
    let data = Vec::new();
    let digest = store.put(&data)?;
    assert!(store.exists(&digest));
    let got = store.get(&digest)?;
    assert_eq!(got, data);
    Ok(())
}

#[test]
fn get_nonexistent_returns_not_found() -> Result<()> {
    let dir = temp_dir();
    let store = store_at(dir.path(), [7u8; 32]);
    let digest = BlobStore::<DevKeyProvider>::digest_of(b"does-not-exist");
    let err = store.get(&digest).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("not found"));
    Ok(())
}
