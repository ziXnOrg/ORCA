//! Blob Store (CAS + zstd + encryption-at-rest)
//!
//! Overview
//! - Content-addressable identity: SHA-256 computed over plaintext bytes.
//! - Determinism: fixed zstd level; AES-256-GCM with nonce = SHA-256(key || digest)[..12].
//! - Atomicity & durability: write to a temporary file, `fsync`, atomic rename, then directory `fsync`.
//! - Fail-closed: any I/O, crypto, or integrity error aborts the operation.
//!
//! Security Model
//! - AES-256-GCM provides confidentiality and integrity at rest.
//! - Nonce derivation is deterministic per (key, digest) to enable idempotent storage and stable ciphertexts.
//!   This is an intentional trade-off to support deduplication; integrity is enforced via AEAD tags and
//!   digest verification on read.
//! - Errors never include secrets; integrity failures do not leak key material.
//!
//! Note: deterministic nonces reveal duplicate content across writes for the same key.
//! For production deployments, plan key rotation with multi-key providers or key IDs to
//! allow decrypting existing blobs during transition windows; this crate does not persist
//! key IDs and assumes the reader can supply historical keys when needed.

//! Determinism Guarantees
//! - `Digest` identity is computed on plaintext only.
//! - Compression uses a fixed zstd level (default 3) for stable output.
//! - Given the same key and bytes, `put` is idempotent and `get` returns identical plaintext.
//!
//! Usage example
//! ```rust
//! use blob_store::{BlobStore, Config, DevKeyProvider};
//! let dir = tempfile::tempdir().unwrap();
//! let cfg = Config::with_root(dir.path().to_path_buf());
//! let key = DevKeyProvider::new([0x11; 32]);
//! let store = BlobStore::new(cfg, key).unwrap();
//! let data = b"hello".to_vec();
//! let digest = store.put(&data).unwrap();
//! assert!(store.exists(&digest));
//! let got = store.get(&digest).unwrap();
//! assert_eq!(got, data);
//! ```

#![warn(missing_docs)]

use std::any::Any;
use std::io::Cursor;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use sha2::digest::{FixedOutput as ShaFixedOutputTrait, Update as ShaUpdateTrait};

/// 32-byte SHA-256 digest type
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// Hex-encoded lowercase string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// Error type for blob store operations
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Underlying IO failure
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Cryptographic failure (AEAD, key, nonce)
    #[error("crypto: {0}")]
    Crypto(String),
    /// Integrity/authentication failure (tamper detected)
    #[error("integrity: authentication tag mismatch or digest mismatch")]
    Integrity,
    /// Blob not found
    #[error("not found")]
    NotFound,
    /// Detected partial/incomplete write artifact
    #[error("partial write detected")]
    PartialWriteDetected,
    /// Wrong key used for decrypting
    #[error("wrong key or decryption failed")]
    WrongKey,
}

/// Key provider trait for encryption-at-rest
pub trait KeyProvider: Send + Sync {
    /// Returns a 32-byte key (AES-256-GCM)
    fn key_bytes(&self) -> [u8; 32];
}

/// In-memory key provider for tests and dev
pub struct DevKeyProvider {
    key: [u8; 32],
}

impl DevKeyProvider {
    /// Create with the provided 32-byte key
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
}

impl KeyProvider for DevKeyProvider {
    fn key_bytes(&self) -> [u8; 32] {
        self.key
    }
}

/// Optional observability hooks (low-cardinality counters and spans).
/// By default these are no-ops. Integrations may register a global observer
/// to emit metrics/traces via OpenTelemetry or other backends.
pub trait BlobStoreObserver: Send + Sync {
    /// Increment logical plaintext bytes accepted by put() operations.
    fn put_bytes(&self, _n: u64) {}
    /// Increment logical plaintext bytes returned by get() operations.
    fn get_bytes(&self, _n: u64) {}
    /// Increment the number of incomplete artifacts cleaned up.
    fn cleanup_count(&self, _n: u64) {}
    /// Start an optional span; dropping ends it.
    fn span(&self, _name: &'static str) -> BlobSpan {
        BlobSpan::noop()
    }
}

/// Guard object for optional spans. Holds a type-erased guard that exits on drop.
pub struct BlobSpan {
    _guard: Option<Box<dyn Any + 'static>>,
}

impl BlobSpan {
    /// Create a no-op span guard.
    pub fn noop() -> Self {
        Self { _guard: None }
    }
    /// Create a span guard from an arbitrary guard object; dropping this will drop the guard.
    pub fn from_guard<G: 'static>(guard: G) -> Self {
        Self { _guard: Some(Box::new(guard)) }
    }
}

impl Drop for BlobSpan {
    fn drop(&mut self) {
        // Dropping `_guard` exits the underlying span if present.
    }
}

struct NoopObserver;
impl BlobStoreObserver for NoopObserver {}

static NOOP_OBSERVER: NoopObserver = NoopObserver;
static OBSERVER: OnceLock<&'static dyn BlobStoreObserver> = OnceLock::new();

/// Register a global observer for blob store metrics/spans (optional).
/// Safe to call at most once; subsequent calls are ignored.
pub fn set_observer(observer: &'static dyn BlobStoreObserver) {
    let _ = OBSERVER.set(observer);
}

fn observer() -> &'static dyn BlobStoreObserver {
    if let Some(o) = OBSERVER.get() {
        *o
    } else {
        &NOOP_OBSERVER
    }
}

// Streaming format header (new in BS2)
const FILE_MAGIC: [u8; 4] = *b"BS2\0";
const FILE_VERSION: u8 = 1;
const CHUNK_SIZE: usize = 64 * 1024; // 64 KiB

fn derive_nonce_prefix(key_bytes: [u8; 32], digest: &Digest) -> [u8; 12] {
    let mut h = sha2::Sha256::default();
    ShaUpdateTrait::update(&mut h, &key_bytes);
    ShaUpdateTrait::update(&mut h, &digest.0);
    let n = ShaFixedOutputTrait::finalize_fixed(h);
    let mut out = [0u8; 12];
    out.copy_from_slice(&n[..12]);
    out
}

struct HashingWriter<W: Write> {
    inner: W,
    hasher: sha2::Sha256,
    count: usize,
}
impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, hasher: sha2::Sha256::default(), count: 0 }
    }
    fn finalize(self) -> (W, [u8; 32], usize) {
        let out = ShaFixedOutputTrait::finalize_fixed(self.hasher);
        let mut d = [0u8; 32];
        d.copy_from_slice(&out);
        (self.inner, d, self.count)
    }
}
impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        ShaUpdateTrait::update(&mut self.hasher, buf);
        self.count += buf.len();
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

// Reader that yields decrypted compressed bytes from an encrypted blob file.
struct DecryptedCompressedReader {
    file: fs::File,
    cipher: Aes256Gcm,
    nonce_prefix: [u8; 12],
    counter: u32,
    buf: Vec<u8>,
    pos: usize,
}
impl DecryptedCompressedReader {
    fn refill(&mut self) -> Result<(), Error> {
        let mut len_buf = [0u8; 4];
        match self.file.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                self.buf.clear();
                self.pos = 0;
                return Ok(());
            }
            Err(e) => return Err(Error::Io(e)),
        }
        let clen = u32::from_be_bytes(len_buf) as usize;
        self.buf.resize(clen, 0);
        self.file.read_exact(&mut self.buf)?;
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[..8].copy_from_slice(&self.nonce_prefix[..8]);
        nonce_bytes[8..].copy_from_slice(&self.counter.to_be_bytes());
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);
        let pt = self
            .cipher
            .decrypt(nonce, self.buf.as_ref())
            .map_err(|_| Error::Crypto("decrypt".into()))?;
        self.buf = pt;
        self.pos = 0;
        self.counter = self.counter.wrapping_add(1);
        Ok(())
    }
}
impl Read for DecryptedCompressedReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.buf.len() {
            // attempt refill
            match self.refill() {
                Ok(()) => {}
                Err(Error::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(0),
                Err(_e) => return Err(io::Error::other("decrypt")),
            }
            if self.buf.is_empty() {
                return Ok(0);
            }
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Blob store configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// Root directory for the blob store
    pub root: PathBuf,
    /// Fixed zstd compression level (deterministic)
    pub zstd_level: i32,
}

impl Config {
    /// Default config with level 3
    pub fn with_root(root: PathBuf) -> Self {
        Self { root, zstd_level: 3 }
    }
}

/// Blob Store API
pub struct BlobStore<K: KeyProvider> {
    cfg: Config,

    key: K,
}

impl<K: KeyProvider> BlobStore<K> {
    /// Create a new store with config and key provider
    pub fn new(cfg: Config, key: K) -> Result<Self, Error> {
        let s = Self { cfg, key };
        // ensure root exists
        std::fs::create_dir_all(&s.cfg.root)?;
        Ok(s)
    }

    /// Compute deterministic blob path from digest (sharded aa/bb/<digest>)
    pub fn path_for(&self, digest_hex: &str) -> PathBuf {
        let (a, b) = (&digest_hex[0..2], &digest_hex[2..4]);
        self.cfg.root.join("sha256").join(a).join(b).join(digest_hex)
    }

    /// Compute the SHA-256 digest for the given bytes (plaintext)
    pub fn digest_of(bytes: &[u8]) -> Digest {
        use sha2::Sha256;
        let mut hasher = Sha256::default();
        ShaUpdateTrait::update(&mut hasher, bytes);
        let out = ShaFixedOutputTrait::finalize_fixed(hasher);
        let mut d = [0u8; 32];
        d.copy_from_slice(&out);
        Digest(d)
    }

    /// Store bytes and return their content digest (CAS). Idempotent on same content.
    pub fn put(&self, bytes: &[u8]) -> Result<Digest, Error> {
        // Delegate to streaming path over a slice reader
        self.put_reader(Cursor::new(bytes))
    }

    /// Streaming put from any reader, with bounded memory and deterministic nonce.
    pub fn put_reader<R: Read>(&self, mut reader: R) -> Result<Digest, Error> {
        let _span = observer().span("blob.put");

        // First pass: hash plaintext and zstd-compress to a temporary compressed file on disk.
        // This avoids buffering the compressed payload in memory.
        let mut hasher = sha2::Sha256::default();

        // Prepare shard dir and final paths
        // We don't know digest yet; write compressed to a temp path under root/tmp
        let tmp_dir = self.cfg.root.join(".tmp");
        fs::create_dir_all(&tmp_dir)?;
        // Create a unique temp file without adding extra dependencies
        let compressed_tmp = {
            let mut i = 0u64;
            loop {
                let candidate = tmp_dir.join(format!("compressed-{}.tmp", i));
                match fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
                    Ok(f) => break (candidate, f),
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                        i = i.wrapping_add(1);
                        continue;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            }
        };
        let (compressed_tmp, comp_file) = compressed_tmp;
        let mut encoder = zstd::stream::write::Encoder::new(comp_file, self.cfg.zstd_level)?;

        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut total_plain: usize = 0;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            total_plain = total_plain.saturating_add(n);
            ShaUpdateTrait::update(&mut hasher, &buf[..n]);
            encoder.write_all(&buf[..n])?;
        }
        let comp_file = encoder.finish()?; // get File back
        comp_file.sync_all()?;

        // Finalize digest and compute final path
        let d_bytes = ShaFixedOutputTrait::finalize_fixed(hasher);
        let mut d = [0u8; 32];
        d.copy_from_slice(&d_bytes);
        let digest = Digest(d);
        let hex = digest.to_hex();
        let final_path = self.path_for(&hex);

        // Idempotency: if exists, record logical bytes and return
        if final_path.exists() {
            observer().put_bytes(total_plain as u64);
            return Ok(digest);
        }

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Encrypt the compressed temp stream into the final .incomplete file with header, then atomic rename.
        let key_bytes = self.key.key_bytes();
        #[allow(deprecated)]
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce_prefix = derive_nonce_prefix(key_bytes, &digest);
        let tmp_path = final_path.with_extension("incomplete");
        {
            let mut out = fs::File::create(&tmp_path)?;
            // Header: magic + version + chunk_size (u32 BE)
            out.write_all(&FILE_MAGIC)?;
            out.write_all(&[FILE_VERSION])?;
            out.write_all(&(CHUNK_SIZE as u32).to_be_bytes())?;

            // Chunked AEAD encrypt: for each plaintext chunk, derive nonce(prefix||counter_be)
            let mut comp_in = fs::File::open(&compressed_tmp)?;
            let mut ring = vec![0u8; CHUNK_SIZE];
            let mut next = vec![0u8; CHUNK_SIZE];
            let mut n = comp_in.read(&mut ring)?;
            let mut counter: u32 = 0;
            if n == 0 {
                // Write one empty chunk to carry an auth tag
                let nonce_bytes = nonce_prefix;
                // last 4 bytes are counter
                out.write_all(&(16u32).to_be_bytes())?; // AES-GCM tag size for empty plaintext
                #[allow(deprecated)]
                let nonce = Nonce::from_slice(&nonce_bytes);
                let ct = cipher
                    .encrypt(nonce, &[][..])
                    .map_err(|_| Error::Crypto("encrypt(empty)".into()))?;
                out.write_all(&ct)?;
            } else {
                loop {
                    let mut nonce_bytes = [0u8; 12];
                    nonce_bytes[..8].copy_from_slice(&nonce_prefix[..8]);
                    nonce_bytes[8..].copy_from_slice(&counter.to_be_bytes());
                    #[allow(deprecated)]
                    let nonce = Nonce::from_slice(&nonce_bytes);
                    let ct = cipher
                        .encrypt(nonce, &ring[..n])
                        .map_err(|_| Error::Crypto("encrypt".into()))?;
                    out.write_all(&(ct.len() as u32).to_be_bytes())?;
                    out.write_all(&ct)?;
                    counter = counter.wrapping_add(1);

                    let m = comp_in.read(&mut next)?;
                    if m == 0 {
                        break;
                    }
                    std::mem::swap(&mut ring, &mut next);
                    n = m;
                }
            }
            out.sync_all()?;
        }
        // Atomic rename with AlreadyExists race handling
        match fs::rename(&tmp_path, &final_path) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if final_path.exists() {
                    let _ = fs::remove_file(&tmp_path);
                } else {
                    return Err(Error::Io(e));
                }
            }
            Err(e) => return Err(Error::Io(e)),
        }
        if let Some(parent) = final_path.parent() {
            if let Ok(dirf) = fs::File::open(parent) {
                let _ = dirf.sync_all();
            }
        }

        // Remove temp compressed
        let _ = fs::remove_file(&compressed_tmp);

        // Record logical plaintext bytes written
        observer().put_bytes(total_plain as u64);
        Ok(digest)
    }

    /// Retrieve plaintext bytes by digest
    pub fn get(&self, digest: &Digest) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        let n = self.get_to_writer(digest, &mut out)?;
        debug_assert_eq!(n, out.len());
        Ok(out)
    }

    /// Streaming read: decrypt+decompress to provided writer, returning bytes written.
    pub fn get_to_writer<W: Write>(&self, digest: &Digest, mut writer: W) -> Result<usize, Error> {
        let _span = observer().span("blob.get");

        let path = self.path_for(&digest.to_hex());
        let mut f = match fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                return if e.kind() == io::ErrorKind::NotFound {
                    Err(Error::NotFound)
                } else {
                    Err(Error::Io(e))
                }
            }
        };

        // Peek header
        let mut header = [0u8; 9];
        let read = f.read(&mut header)?;
        if read < header.len() || header[..4] != FILE_MAGIC {
            // Legacy format: read full file into memory and fall back to single-shot decrypt+decompress
            let mut enc = Vec::with_capacity(fs::metadata(&path)?.len() as usize);
            if read > 0 {
                enc.extend_from_slice(&header[..read]);
            }
            f.read_to_end(&mut enc)?;

            let key_bytes = self.key.key_bytes();
            #[allow(deprecated)]
            let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
            let cipher = Aes256Gcm::new(key);
            let nonce_prefix = derive_nonce_prefix(key_bytes, digest);
            #[allow(deprecated)]
            let nonce = Nonce::from_slice(&nonce_prefix);
            let compressed = cipher
                .decrypt(nonce, enc.as_ref())
                .map_err(|_| Error::Crypto("decrypt(legacy)".into()))?;

            // Decompress and stream to hashing writer via read::Decoder
            let mut dec = zstd::stream::read::Decoder::new(Cursor::new(compressed))
                .map_err(|_| Error::Integrity)?;
            let mut hw = HashingWriter::new(&mut writer);
            let count = io::copy(&mut dec, &mut hw).map_err(|_| Error::Integrity)? as usize;
            let (_w, d_bytes, _c) = hw.finalize();
            if Digest(d_bytes) != *digest {
                return Err(Error::Integrity);
            }
            observer().get_bytes(count as u64);
            return Ok(count);
        }

        let version = header[4];
        if version != FILE_VERSION {
            return Err(Error::Integrity);
        }
        let mut sz = [0u8; 4];
        sz.copy_from_slice(&header[5..9]);
        let _chunk_size = u32::from_be_bytes(sz) as usize;

        let key_bytes = self.key.key_bytes();
        #[allow(deprecated)]
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce_prefix = derive_nonce_prefix(key_bytes, digest);
        // Build decrypted-compressed reader and pipe through zstd read::Decoder into hashing writer
        let reader = DecryptedCompressedReader {
            file: f,
            cipher,
            nonce_prefix,
            counter: 0,
            buf: Vec::new(),
            pos: 0,
        };
        let mut dec = zstd::stream::read::Decoder::new(reader).map_err(|_| Error::Integrity)?;
        let mut hw = HashingWriter::new(&mut writer);
        let count = io::copy(&mut dec, &mut hw).map_err(|_| Error::Integrity)? as usize;
        let (_w, d_bytes, _c) = hw.finalize();
        if Digest(d_bytes) != *digest {
            return Err(Error::Integrity);
        }
        observer().get_bytes(count as u64);
        Ok(count)
    }

    /// Return true if a blob with this digest is present
    pub fn exists(&self, digest: &Digest) -> bool {
        self.path_for(&digest.to_hex()).exists()
    }

    /// Remove any .incomplete artifacts under root; return count removed
    pub fn cleanup_incomplete(&self) -> Result<usize, Error> {
        let _span = observer().span("blob.cleanup");

        fn walk(dir: &Path, count: &mut usize) -> io::Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let _ = walk(&path, count);
                } else if path.extension().map(|e| e == "incomplete").unwrap_or(false) {
                    fs::remove_file(&path)?;
                    *count += 1;
                }
            }
            Ok(())
        }
        let mut removed = 0usize;
        let root = self.cfg.root.join("sha256");
        if root.exists() {
            let _ = walk(&root, &mut removed);
        }
        observer().cleanup_count(removed as u64);

        Ok(removed)
    }
}

/// Helper to build a deterministic test buffer of given size
pub fn deterministic_bytes(len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    for i in 0..len {
        v.push((i as u8).wrapping_mul(37).wrapping_add(11));
    }
    v
}
