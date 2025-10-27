//! Blob Store MVP (CAS + zstd + encryption-at-rest)
//!
//! Safety-critical defaults:
//! - Deterministic content identity (SHA-256 of plaintext)
//! - Fail-closed on key/config/IO errors
//! - Atomic writes with temp file + rename (to be implemented)
//! - Encryption-at-rest (AES-GCM) with deterministic nonce derived from digest (to be implemented)
//! - Compression at rest via zstd with fixed level (to be implemented)

#![warn(missing_docs)]

use std::path::{Path, PathBuf};

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
    pub fn new(key: [u8; 32]) -> Self { Self { key } }
}

impl KeyProvider for DevKeyProvider {
    fn key_bytes(&self) -> [u8; 32] { self.key }
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
    pub fn with_root(root: PathBuf) -> Self { Self { root, zstd_level: 3 } }
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
        use sha2::{Digest as _, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let out = hasher.finalize();
        let mut d = [0u8; 32];
        d.copy_from_slice(&out);
        Digest(d)
    }

    /// Store bytes and return their content digest (CAS). Idempotent on same content.
    pub fn put(&self, _bytes: &[u8]) -> Result<Digest, Error> {
        // RED-phase stub: not implemented yet
        Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "not implemented (RED)",
        )))
    }

    /// Retrieve plaintext bytes by digest
    pub fn get(&self, _digest: &Digest) -> Result<Vec<u8>, Error> {
        // RED-phase stub
        Err(Error::NotFound)
    }

    /// Return true if a blob with this digest is present
    pub fn exists(&self, digest: &Digest) -> bool {
        self.path_for(&digest.to_hex()).exists()
    }

    /// Remove any incomplete artifacts; return count removed
    pub fn cleanup_incomplete(&self) -> Result<usize, Error> {
        // RED-phase stub
        Ok(0)
    }
}

/// Helper to build a deterministic test buffer of given size
pub fn deterministic_bytes(len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    for i in 0..len { v.push((i as u8).wrapping_mul(37).wrapping_add(11)); }
    v
}

