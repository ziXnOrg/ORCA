// Robustness tests for BS2 read path: header/chunk bounds and memory.
// Property tests are kept light for CI and complemented by an ignored manual memory harness.

#![cfg(test)]

use blob_store::{BlobStore, Config, DevKeyProvider, Error};
use std::io::Write;
use std::path::PathBuf;

fn make_store() -> (tempfile::TempDir, BlobStore<DevKeyProvider>) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config { root: PathBuf::from(dir.path()), zstd_level: 3 };
    let store: BlobStore<DevKeyProvider> =
        BlobStore::new(cfg, DevKeyProvider::new([7u8; 32])).unwrap();
    (dir, store)
}

fn write_at_path(path: &std::path::Path, bytes: &[u8]) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().ok();
}

#[test]
fn rejects_header_chunk_size_zero() {
    let (_dir, store) = make_store();
    let digest = blob_store::BlobStore::<DevKeyProvider>::digest_of(b"dummy");
    let path = store.path_for(&digest.to_hex());

    // Header: magic + version + chunk_size=0
    let mut file = Vec::new();
    file.extend_from_slice(b"BS2\0");
    file.push(1u8); // version
    file.extend_from_slice(&(0u32).to_be_bytes());
    write_at_path(&path, &file);

    let mut sink = std::io::sink();
    let res = store.get_to_writer(&digest, &mut sink);
    assert!(matches!(res, Err(Error::Integrity)));
}

#[test]
fn rejects_chunk_len_over_bound() {
    let (_dir, store) = make_store();
    let digest = blob_store::BlobStore::<DevKeyProvider>::digest_of(b"dummy2");
    let path = store.path_for(&digest.to_hex());

    let chunk_size = 4096u32; // small for test
    let clen = chunk_size as usize + 16 + 1; // > chunk_size + AEAD_TAG_SIZE

    let mut file = Vec::new();
    file.extend_from_slice(b"BS2\0");
    file.push(1u8);
    file.extend_from_slice(&chunk_size.to_be_bytes());
    file.extend_from_slice(&(clen as u32).to_be_bytes());
    file.extend(std::iter::repeat(0u8).take(clen)); // bogus ciphertext
    write_at_path(&path, &file);

    let mut sink = std::io::sink();
    let res = store.get_to_writer(&digest, &mut sink);
    // Any integrity failure is acceptable; critical requirement is that we do not
    // allocate beyond the declared chunk bound. The implementation returns Integrity.
    assert!(matches!(res, Err(Error::Integrity)));
}

fn rss_kb() -> Option<usize> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let pid = std::process::id();
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        let kb = s.trim().parse::<usize>().ok()?;
        return Some(kb);
    }
    #[allow(unreachable_code)]
    None
}

#[test]
#[ignore]
fn streaming_get_memory_bound_manual() {
    if std::env::var("LARGE_BLOB_TEST").ok().as_deref() != Some("1") {
        eprintln!("skipped; set LARGE_BLOB_TEST=1 to run");
        return;
    }
    let rss_limit_kb: usize =
        std::env::var("RSS_LIMIT_KB").ok().and_then(|v| v.parse().ok()).unwrap_or(32 * 1024);

    let (_dir, store) = make_store();
    let digest = blob_store::BlobStore::<DevKeyProvider>::digest_of(b"dummy3");
    let path = store.path_for(&digest.to_hex());

    // Craft a file declaring a huge clen without providing the bytes to force old
    // implementations to allocate before failing read_exact. New code should reject
    // before allocation.
    let declared_chunk = 64 * 1024u32; // 64 KiB header chunk size
    let huge_clen = 128 * 1024 * 1024 + 17; // 128 MiB + tag + 1

    let mut file = Vec::new();
    file.extend_from_slice(b"BS2\0");
    file.push(1u8);
    file.extend_from_slice(&declared_chunk.to_be_bytes());
    file.extend_from_slice(&(huge_clen as u32).to_be_bytes()); // truncated, sufficient to trigger path
    write_at_path(&path, &file);

    let before = rss_kb();
    let mut sink = std::io::sink();
    let _ = store.get_to_writer(&digest, &mut sink); // expect Err; interested in memory behavior
    let after = rss_kb();

    if let (Some(b), Some(a)) = (before, after) {
        let peak = a.max(b);
        eprintln!("GET RSS sampled (KB): before={}, after={}, limit={}", b, a, rss_limit_kb);
        assert!(peak <= rss_limit_kb, "peak RSS {} KB exceeded limit {} KB", peak, rss_limit_kb);
    } else {
        eprintln!("RSS sampling unavailable on this platform");
    }
}
