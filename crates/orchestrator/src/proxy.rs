//! Proxy/capture helpers for external I/O (HTTP/gRPC).
//! GREEN implemented server-side stubs; REFACTOR adds real SHA-256 and client-side layer skeleton.

use serde_json::{Map as JsonMap, Value as JsonValue};
use tonic::metadata::MetadataMap;

// For client-side redaction helpers
use http::HeaderMap;

// SHA-256 digest (streaming) and hex encoding
use sha2::{Digest, Sha256};

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

/// Redact from HTTP header map (client-side path); only builds a map when any sensitive key is present.
pub fn redacted_headers_from_http(headers: &HeaderMap) -> Option<JsonMap<String, JsonValue>> {
    let mut out = JsonMap::new();
    let mut found = false;
    for key in ["authorization", "cookie", "x-api-key"] {
        if headers.get(key).is_some() {
            out.insert(key.to_string(), JsonValue::String("[REDACTED]".into()));
            found = true;
        }
    }
    if found {
        Some(out)
    } else {
        None
    }
}

/// Real SHA-256 with streaming updates and lowercase hex output.
/// Chunked updates avoid large transient buffers; input is borrowed, no extra allocations.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const CHUNK: usize = 64 * 1024; // 64 KiB default (validated via benchmarks)
    let mut hasher = Sha256::new();
    if bytes.len() <= CHUNK {
        hasher.update(bytes);
    } else {
        for chunk in bytes.chunks(CHUNK) {
            hasher.update(chunk);
        }
    }

    // ===== Client-side capture layer skeleton (not yet wired) =====
    use http::{Request, Response};
    use std::task::{Context, Poll};
    use tonic::body::BoxBody;
    use tower::{Layer, Service};

    #[allow(dead_code)]
    #[derive(Debug, Clone, Default)]
    pub struct ProxyCaptureLayer;

    impl<S> Layer<S> for ProxyCaptureLayer {
        type Service = ProxyCapturedChannel<S>;
        fn layer(&self, inner: S) -> Self::Service {
            ProxyCapturedChannel { inner }
        }
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    pub struct ProxyCapturedChannel<S> {
        inner: S,
    }

    impl<S> Service<Request<BoxBody>> for ProxyCapturedChannel<S>
    where
        S: Service<Request<BoxBody>, Response = Response<tonic::transport::Body>> + Send,
        S::Future: Send + 'static,
    {
        type Response = Response<tonic::transport::Body>;
        type Error = S::Error;
        type Future = S::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        fn call(&mut self, req: Request<BoxBody>) -> Self::Future {
            // NOTE: REFACTOR wiring will emit ExternalIOStarted/Finished around this call
            // with deterministic request_id and redacted headers.
            self.inner.call(req)
        }
    }

    /// Builder to construct a captured Channel from a tonic Channel. Future work will
    /// accept handles for WAL and metrics; for now this is a no-op wrapper.
    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    pub struct CapturedChannelBuilder {
        inner: tonic::transport::Channel,
    }

    #[allow(dead_code)]
    impl CapturedChannelBuilder {
        pub fn new(inner: tonic::transport::Channel) -> Self {
            Self { inner }
        }
        pub fn build(self) -> ProxyCapturedChannel<tonic::transport::Channel> {
            ProxyCapturedChannel { inner: self.inner }
        }
    }

    let digest = hasher.finalize();
    hex::encode(digest)
}
