// Memory bounds harness for streaming put/get. Ignored by default.
// Enable with: LARGE_BLOB_TEST=1 cargo test -p blob_store --test streaming_memory -- --nocapture

#![cfg(test)]

use blob_store::{BlobStore, Config, DevKeyProvider};
use std::io::Read;
use std::path::PathBuf;

struct DeterministicReader {
    remaining: usize,
    idx: usize,
}
impl DeterministicReader {
    fn new(len: usize) -> Self {
        Self { remaining: len, idx: 0 }
    }
}
impl Read for DeterministicReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let n = buf.len().min(self.remaining);
        for b in buf.iter_mut().take(n) {
            *b = (self.idx as u8).wrapping_mul(37).wrapping_add(11);
            self.idx += 1;
        }
        self.remaining -= n;
        Ok(n)
    }
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
fn streaming_put_memory_bound_manual() {
    if std::env::var("LARGE_BLOB_TEST").ok().as_deref() != Some("1") {
        eprintln!("skipped; set LARGE_BLOB_TEST=1 to run");
        return;
    }
    let target_bytes = std::env::var("LARGE_BLOB_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000_000);
    let rss_limit_kb: usize =
        std::env::var("RSS_LIMIT_KB").ok().and_then(|v| v.parse().ok()).unwrap_or(32 * 1024);

    let dir = tempfile::tempdir().unwrap();
    let cfg = Config { root: PathBuf::from(dir.path()), zstd_level: 3 };
    let store: BlobStore<DevKeyProvider> =
        BlobStore::new(cfg, DevKeyProvider::new([5u8; 32])).unwrap();

    let before = rss_kb();
    let digest = store.put_reader(DeterministicReader::new(target_bytes)).expect("put_reader");
    let after = rss_kb();

    if let (Some(b), Some(a)) = (before, after) {
        let peak = a.max(b); // crude; real peak not measured; best-effort sampling
        eprintln!("RSS sampled (KB): before={}, after={}, limit={}", b, a, rss_limit_kb);
        assert!(peak <= rss_limit_kb, "peak RSS {} KB exceeded limit {} KB", peak, rss_limit_kb);
    } else {
        eprintln!("RSS sampling unavailable on this platform");
    }

    // Optional: validate read path without allocating full buffer by discarding bytes
    struct SinkWriter(usize);
    impl std::io::Write for SinkWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = SinkWriter(0);
    let n = store.get_to_writer(&digest, &mut sink).expect("get_to_writer");
    assert_eq!(n, sink.0);
}
