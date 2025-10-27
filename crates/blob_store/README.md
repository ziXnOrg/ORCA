# Blob Store (BS2 streaming, bounded-memory)

Content-addressable blob store with:
- SHA-256 digest computed over plaintext
- zstd compression (fixed level)
- AES-256-GCM encryption-at-rest with deterministic nonce scheme
- Atomic writes (temp file + fsync + rename)
- Fail-closed error handling and optional observability hooks

## BS2 Streaming Format
Header (9 bytes):
- magic: "BS2\0" (4)
- version: 1 (1)
- chunk_size: u32 (big-endian) (4)

Body:
- Repeated `[len_be (u32)][ciphertext bytes]`
- Each ciphertext is an AEAD of up to `chunk_size` bytes of zstd-compressed data
- Each chunk carries its own authentication tag

Nonce scheme:
- prefix = SHA256(key || digest)[..12]
- nonce = prefix[..8] || counter_be32, starting from 0 per chunk

Determinism:
- Plaintext digest is SHA-256 over uncompressed bytes
- zstd level is fixed (default: 3)
- With the same key and input, digests and ciphertext are stable

Memory bounds:
- Working set is O(chunk_size) on both put and get paths (default 64 KiB)
- No unbounded allocations on control paths; large payloads are streamed via temp files

Legacy compatibility:
- Blobs without the BS2 header are treated as legacy: a single-shot AEAD over a full compressed stream
- Reads remain supported; new writes always produce BS2

## Usage

Create a store (dev key shown for example only):

```rust
use blob_store::{BlobStore, Config, DevKeyProvider};
let dir = tempfile::tempdir()?;
let cfg = Config::with_root(dir.path().to_path_buf());
let key = DevKeyProvider::new([0x11; 32]);
let store = BlobStore::new(cfg, key)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Streaming put from any reader

```rust
use std::io::{self, Read};
use blob_store::BlobStore;

// Example reader: a file or any Read
let data = vec![42u8; 10 * 1024 * 1024]; // 10 MiB
let digest = store.put_reader(std::io::Cursor::new(data))?;
assert!(store.exists(&digest));
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Streaming get to any writer

```rust
use std::io::Write;
let mut out = Vec::new();
let n = store.get_to_writer(&digest, &mut out)?;
assert_eq!(n, out.len());
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Large blobs (bounded memory)

```rust
use std::io::{self, Read};
struct DeterministicReader { remaining: usize }
impl Read for DeterministicReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 { return Ok(0); }
        let n = buf.len().min(self.remaining);
        for (i, b) in buf[..n].iter_mut().enumerate() { *b = (i as u8).wrapping_mul(37).wrapping_add(11); }
        self.remaining -= n;
        Ok(n)
    }
}
let reader = DeterministicReader { remaining: 1_000_000_000 }; // 1 GiB
let digest = store.put_reader(reader)?; // memory remains O(chunk_size)
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Observability
- Integrations may register a global `BlobStoreObserver` to emit counters and spans
- Existing counters: `put_bytes` and `get_bytes` (logical plaintext)

## Errors and safety
- Fail-closed on I/O, crypto, and integrity errors
- No secrets in errors; deterministic behavior preserved

---
For details, see inline rustdoc in `src/lib.rs`.

