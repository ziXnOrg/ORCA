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
- Read path enforces header-declared chunk_size and rejects any chunk length > chunk_size + 16 (AEAD tag)
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



## Operational Runbook

### Key handling best practices
- DevKeyProvider is for local/dev only. For production, implement `KeyProvider` backed by a secure KMS/HSM or sealed key store.
- Keys must be 32-byte AES-256-GCM keys; zeroize in memory when possible and avoid logging or serializing.
- Scope keys per tenant or security domain to bound blast radius; rotate keys periodically.

### Key rotation and migration strategy
- New writes should switch to a new key version K_{n+1} while reads continue to support K_n.
- Store only digests on references; the blob store derives nonces deterministically from (key,digest) without persisting nonces.
- Rotation options:
  - Lazy migration: decrypt with old key on read, re-encrypt on write/update path.
  - Bulk migration: offline job reads → decrypts with K_n → re-encrypts with K_{n+1} (verify digest invariant).
- Keep a short list of active keys in your `KeyProvider` and fail-closed if no key can decrypt.

### Failure modes and recovery
- Partial writes: crashes leave `*.incomplete` temp files; call `cleanup_incomplete()` periodically or at startup.
- Corruption: AEAD integrity failures or BS2 header/clen bound violations return `Error::Integrity`.
- Wrong key: returns `Error::WrongKey` (or `Integrity` depending on failure stage) — never log secrets.
- Missing blobs: `Error::NotFound` — callers may choose to re-materialize from source or mark reference invalid.
- Atomicity: writes are fsync'ed then atomically renamed; treat `AlreadyExists` on rename as success if the final path exists.

### Determinism guarantees
- Plaintext digest: SHA-256 over uncompressed bytes; stable across hosts.
- Compression: fixed zstd level (default: 3) for deterministic compressed output.
- Nonces: deterministic derivation `prefix = SHA256(key || digest)[..12]`, per-chunk `nonce = prefix[..8] || counter_be32`.
- With same input and key, ciphertext is stable; integrity verified by AEAD tags and final plaintext digest.

### Compatibility notes
- BS2 is the default write format: `magic="BS2\0"`, `version=1`, `chunk_size: u32`.
- Legacy fallback: blobs without the BS2 header are treated as a single-shot AEAD over the full compressed stream for reads only.
- New writes always produce BS2; keep read-path legacy fallback to preserve backward compatibility.

### Performance characteristics
- Bounded memory: working set is O(chunk_size) for put/get (default ~64 KiB); no unbounded allocations on control paths.
- Streaming behavior: large blobs are compressed to a temp file, then encrypted and chunked during finalize; reads stream decrypt+decompress per chunk.
- Robustness: read path enforces header-declared `chunk_size` and rejects any chunk length > `chunk_size + AEAD_TAG_SIZE`.
