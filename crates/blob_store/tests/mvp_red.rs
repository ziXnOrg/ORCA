// MVP acceptance tests for CAS + zstd + encryption-at-rest
// Note: The current implementation may already satisfy these tests.

use blob_store::{deterministic_bytes, BlobStore, Config, DevKeyProvider, Error};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn store_at(path: &std::path::Path, key: [u8; 32]) -> BlobStore<DevKeyProvider> {
    let cfg = Config { root: PathBuf::from(path), zstd_level: 3 };
    let kp = DevKeyProvider::new(key);
    BlobStore::new(cfg, kp).unwrap()
}

#[test]
fn cas_semantics_digest_stability_and_idempotent_put() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = store_at(dir.path(), [0xAA; 32]);
    let data = b"lorem ipsum dolor sit amet".to_vec();

    let d_expected = BlobStore::<DevKeyProvider>::digest_of(&data);
    let d1 = store.put(&data)?;
    let d2 = store.put(&data)?;
    assert_eq!(d_expected, d1, "digest computed by API must match expected SHA-256");
    assert_eq!(d1, d2, "put must be idempotent on identical content");
    Ok(())
}

#[test]
fn zstd_round_trip_and_ratio_on_compressible_payload() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = store_at(dir.path(), [0xBB; 32]);
    // Highly compressible payload
    let payload = vec![b'A'; 2 * 1024 * 1024]; // 2 MiB of 'A'
    let digest = store.put(&payload)?;
    let shard_path = store.path_for(&digest.to_hex());

    // On-disk file should be significantly smaller than plaintext (zstd + AEAD overhead tolerated)
    let disk_len = fs::metadata(&shard_path)?.len() as usize;
    let ratio = disk_len as f64 / payload.len() as f64;
    assert!(ratio < 0.20, "expected strong compression, got ratio={ratio:.3}");

    // Round-trip integrity
    let got = store.get(&digest)?;
    assert_eq!(got, payload);
    Ok(())
}

#[test]
fn encryption_at_rest_ciphertext_properties() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store1 = store_at(dir.path(), [0xCC; 32]);
    let store2 = store_at(dir.path(), [0xCC; 32]); // same key
    let store_other_key = store_at(dir.path(), [0xCD; 32]);

    let data = deterministic_bytes(64 * 1024);
    let d = store1.put(&data)?;

    // Header should be BS2 and not contain plaintext
    let path = store1.path_for(&d.to_hex());
    let bytes = fs::read(&path)?;
    assert!(bytes.starts_with(b"BS2\0"), "missing BS2 header");
    assert!(
        !bytes.windows(8).any(|w| w == &data[..8]),
        "ciphertext should not contain plaintext sequences"
    );

    // Stable ciphertext for same input/key across puts
    let d2 = store2.put(&data)?;
    assert_eq!(d, d2);
    let bytes2 = fs::read(store2.path_for(&d2.to_hex()))?;
    assert_eq!(bytes, bytes2, "ciphertext must be deterministic for same (key, digest)");

    // Different key must fail to decrypt
    let err = store_other_key.get(&d).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("wrong key") || s.contains("crypto") || s.contains("integrity"));
    Ok(())
}

#[test]
fn error_cases_missing_and_corrupted() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = store_at(dir.path(), [0xEE; 32]);

    // Missing
    let missing = BlobStore::<DevKeyProvider>::digest_of(b"does-not-exist");
    match store.get(&missing) {
        Err(Error::NotFound) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }

    // Corrupted (flip a byte)
    let d = store.put(&deterministic_bytes(16 * 1024))?;
    let path = store.path_for(&d.to_hex());
    let mut c = fs::read(&path)?;
    let mid = c.len() / 2;
    c[mid] ^= 0x5A;
    fs::write(&path, &c)?;
    let e = store.get(&d).unwrap_err();
    let s = format!("{e}");
    assert!(s.contains("integrity") || s.contains("crypto"));
    Ok(())
}

#[test]
fn streaming_compatibility_digest_and_round_trip() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = store_at(dir.path(), [0xAB; 32]);

    let data = deterministic_bytes(128 * 1024);
    let d_buf = store.put(&data)?;
    let d_stream = store.put_reader(Cursor::new(data.clone()))?;
    assert_eq!(d_buf, d_stream, "digest equality between buffered and streaming put");

    let got = store.get(&d_stream)?;
    assert_eq!(got, data);
    Ok(())
}
