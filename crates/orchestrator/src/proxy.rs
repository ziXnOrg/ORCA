//! Proxy/capture helpers for external I/O (HTTP/gRPC).
//! Skeleton implementation to support WAL stub capture and redaction in GREEN.

use serde_json::{Map as JsonMap, Value as JsonValue};
use tonic::metadata::MetadataMap;

pub fn capture_enabled() -> bool {
    std::env::var("ORCA_CAPTURE_EXTERNAL_IO").ok().as_deref() == Some("1")
}

pub fn bypass_to_direct() -> bool {
    std::env::var("ORCA_BYPASS_TO_DIRECT").ok().as_deref() == Some("1")
}

pub fn fail_inject_enabled() -> bool {
    std::env::var("ORCA_CAPTURE_FAIL_INJECT").ok().as_deref() == Some("1")
}

/// Redact sensitive headers according to a simple allowlist policy.
/// Currently redacts: authorization, cookie, x-api-key.
pub fn redacted_headers(md: &MetadataMap) -> JsonMap<String, JsonValue> {
    let mut out = JsonMap::new();
    for key in ["authorization", "cookie", "x-api-key"] {
        if md.get(key).is_some() {
            out.insert(key.to_string(), JsonValue::String("[REDACTED]".into()));
        }
    }
    out
}

/// Placeholder for SHA-256 digest computation; returns a 64-hex string (all zeros)
/// to satisfy GREEN tests without adding new dependencies. Replace with real SHA-256
/// in PRE-REFACTOR/REFACTOR.
pub fn body_digest_sha256_stub(_bytes: &[u8]) -> String {
    "0".repeat(64)
}
