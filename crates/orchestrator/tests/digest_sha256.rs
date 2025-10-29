//! Unit test for sha256_hex: verifies stable digest for >5MB payload and chunked vs single-update equivalence.

use orchestrator::proxy::sha256_hex;
use sha2::{Digest, Sha256};

#[test]
fn sha256_large_payload_chunked_matches_single_update() {
    // Build a >5MB deterministic payload
    let len = 6 * 1024 * 1024; // 6 MiB
    let mut data = vec![0u8; len];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i as u32 % 251) as u8; // deterministic non-trivial pattern
    }

    // Expected via single update
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let expected = hex::encode(hasher.finalize());

    let got = sha256_hex(&data);
    assert_eq!(got, expected, "chunked sha256_hex must equal single-update digest");
}
