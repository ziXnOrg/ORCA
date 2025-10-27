//! Blob Store MVP (CAS + zstd + encryption-at-rest)
//!
//! Safety-critical defaults:
//! - Deterministic content identity (SHA-256 of plaintext)
//! - Fail-closed on key/config/IO errors
//! - Atomic writes with temp file + rename (to be implemented)
//! - Encryption-at-rest (AES-GCM) with deterministic nonce derived from digest (to be implemented)
//! - Compression at rest via zstd with fixed level (to be implemented)

#![warn(missing_docs)]

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use std::io::Cursor;

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use zstd::stream::{decode_all as zstd_decode_all, encode_all as zstd_encode_all};

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
        use sha2::{Digest as _, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let out = hasher.finalize();
        let mut d = [0u8; 32];
        d.copy_from_slice(&out);
        Digest(d)
    }

    /// Store bytes and return their content digest (CAS). Idempotent on same content.
    pub fn put(&self, bytes: &[u8]) -> Result<Digest, Error> {
        // Compute plaintext digest (identity)
        let digest = Self::digest_of(bytes);
        let hex = digest.to_hex();
        let final_path = self.path_for(&hex);

        // Ensure shard directory exists
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Idempotent: if already present, return digest
        if final_path.exists() {
            return Ok(digest);
        }

        // Compress deterministically
        let compressed = zstd_encode_all(Cursor::new(bytes), self.cfg.zstd_level)?;

        // Derive deterministic nonce from key + digest
        use sha2::Digest as _;
        let key_bytes = self.key.key_bytes();
        let mut h = sha2::Sha256::new();
        h.update(key_bytes);
        h.update(digest.0);
        let n = h.finalize();
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes.copy_from_slice(&n[..12]);

        // Encrypt compressed payload
        #[allow(deprecated)]
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, compressed.as_ref())
            .map_err(|_| Error::Crypto("encrypt".to_string()))?;

        // Write atomically: tmp -> fsync -> rename -> fsync dir
        let tmp_path = final_path.with_extension("incomplete");
        {
            let mut f = fs::File::create(&tmp_path)?;
            f.write_all(&ciphertext)?;
            f.sync_all()?;
        }
        fs::rename(&tmp_path, &final_path)?;
        if let Some(parent) = final_path.parent() {
            if let Ok(dirf) = fs::File::open(parent) {
                let _ = dirf.sync_all();
            }
        }

        Ok(digest)
    }

    /// Retrieve plaintext bytes by digest
    pub fn get(&self, digest: &Digest) -> Result<Vec<u8>, Error> {
        let path = self.path_for(&digest.to_hex());
        let enc = match fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                return if e.kind() == io::ErrorKind::NotFound {
                    Err(Error::NotFound)
                } else {
                    Err(Error::Io(e))
                }
            }
        };

        // Re-derive deterministic nonce from key + digest
        use sha2::Digest as _;
        let key_bytes = self.key.key_bytes();
        let mut h = sha2::Sha256::new();
        h.update(key_bytes);
        h.update(digest.0);
        let n = h.finalize();
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes.copy_from_slice(&n[..12]);

        #[allow(deprecated)]
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);

        let compressed = cipher
            .decrypt(nonce, enc.as_ref())
            .map_err(|_| Error::Integrity)?;

        let plain = zstd_decode_all(Cursor::new(compressed)).map_err(|_| Error::Integrity)?;

        if Self::digest_of(&plain) != *digest {
            return Err(Error::Integrity);
        }
        Ok(plain)
    }

    /// Return true if a blob with this digest is present
    pub fn exists(&self, digest: &Digest) -> bool {
        self.path_for(&digest.to_hex()).exists()
    }

    /// Remove any .incomplete artifacts under root; return count removed
    pub fn cleanup_incomplete(&self) -> Result<usize, Error> {
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
