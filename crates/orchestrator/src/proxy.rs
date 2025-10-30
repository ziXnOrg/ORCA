//! Proxy/capture helpers for external I/O (HTTP/gRPC).
//! GREEN implemented server-side stubs; adds client-side layer wiring behind `capture` feature.

use serde_json::{Map as JsonMap, Value as JsonValue};
use tonic::metadata::MetadataMap;

// For client-side redaction helpers
use http::HeaderMap;

// SHA-256 digest (streaming) and hex encoding
use sha2::{Digest, Sha256};

use event_log::JsonlEventLog;
use std::sync::{OnceLock, RwLock};

// Global capture log sink for client-side capture (tests/bench can set/reset).
static CAPTURE_LOG: OnceLock<RwLock<Option<JsonlEventLog>>> = OnceLock::new();

/// Set/replace the global capture log sink used by client-side capture.
pub fn set_capture_log(log: JsonlEventLog) {
    let cell = CAPTURE_LOG.get_or_init(|| RwLock::new(None));
    *cell.write().unwrap() = Some(log);
}

/// Get a clone of the current capture log sink if configured.
fn capture_log_clone() -> Option<JsonlEventLog> {
    CAPTURE_LOG.get().and_then(|l| l.read().unwrap().clone())
}

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
    let digest = hasher.finalize();
    hex::encode(digest)
}

// ===== Client-side capture layer (wired behind `capture` feature) =====
use http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tonic::body::BoxBody;

#[derive(Debug, Clone, Default)]
pub struct ProxyCaptureLayer;

impl<S> Layer<S> for ProxyCaptureLayer {
    type Service = ProxyCapturedChannel<S>;
    fn layer(&self, inner: S) -> Self::Service {
        ProxyCapturedChannel {
            inner,
            scheme: "grpc".to_string(),
            host: "unknown".to_string(),
            port: 0,
            log: capture_log_clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyCapturedChannel<S> {
    inner: S,
    // Endpoint metadata captured at build time (builder can set real values)
    scheme: String,
    host: String,
    port: u16,
    // Cached capture sink to avoid per-request RwLock reads
    log: Option<JsonlEventLog>,
}

#[cfg(feature = "capture")]
impl<S> Service<Request<BoxBody>> for ProxyCapturedChannel<S>
where
    S: Service<Request<BoxBody>, Response = Response<tonic::transport::Body>> + Send,
    S::Future: Send + 'static,
{
    type Response = Response<tonic::transport::Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<tonic::transport::Body>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<BoxBody>) -> Self::Future {
        // Only emit when runtime capture enabled and a log sink is configured.
        let log = if capture_enabled() { self.log.clone() } else { None };
        let t0 = crate::clock::process_clock().now_ms();
        let rid = format!("R{}", orca_core::ids::next_monotonic_id());

        if let Some(logc) = log.clone() {
            // Extract method and headers; redaction only when sensitive headers present.
            let method_path = req.uri().path().to_string();
            let headers_map = redacted_headers_from_http(req.headers()).unwrap_or_default();
            let started = serde_json::json!({
                "event": "external_io_started",
                "system": "grpc",
                "direction": "client",
                "scheme": self.scheme,
                "host": self.host,
                "port": self.port,
                "method": method_path,
                "request_id": rid,
                "headers": serde_json::Value::Object(headers_map),
                "body_digest_sha256": sha256_hex(&[]),
            });
            let _ = logc.append(orca_core::ids::next_monotonic_id(), t0, &started);
        }

        let fut = self.inner.call(req);
        Box::pin(async move {
            let res = fut.await;
            if let Some(logc) = log.clone() {
                let t1 = crate::clock::process_clock().now_ms();
                let status = if res.is_ok() { "ok" } else { "error" };
                let finished = serde_json::json!({
                    "event": "external_io_finished",
                    "request_id": rid,
                    "status": status,
                    "duration_ms": t1.saturating_sub(t0),
                });
                let _ = logc.append(orca_core::ids::next_monotonic_id(), t1, &finished);
                #[cfg(feature = "otel")]
                {
                    let metric = serde_json::json!({
                        "metric":"proxy.capture.duration_ms", "value_ms": t1.saturating_sub(t0),
                        "attrs": {"system":"grpc","direction":"client","status": status}
                    });
                    let _ = logc.append(orca_core::ids::next_monotonic_id(), t1, &metric);
                }
            }
            res
        })
    }
}

#[cfg(not(feature = "capture"))]
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
        self.inner.call(req)
    }
}

/// Convenience helpers for tests/bench to avoid exposing internal types directly.
pub fn wrap_service<S>(inner: S) -> ProxyCapturedChannel<S> {
    ProxyCaptureLayer::default().layer(inner)
}

pub fn test_set_capture_log(log: JsonlEventLog) {
    set_capture_log(log)
}

/// Builder to construct a captured Channel from a tonic Channel.
#[derive(Debug, Clone)]
pub struct CapturedChannelBuilder {
    inner: tonic::transport::Channel,
    scheme: String,
    host: String,
    port: u16,
}

impl CapturedChannelBuilder {
    /// Create a builder from a connected tonic Channel.
    pub fn new(inner: tonic::transport::Channel) -> Self {
        Self { inner, scheme: "grpc".into(), host: "unknown".into(), port: 0 }
    }

    /// Optionally set endpoint parts (scheme, host, port) if known.
    pub fn endpoint_parts(mut self, scheme: &str, host: &str, port: u16) -> Self {
        self.scheme = scheme.to_string();
        self.host = host.to_string();
        self.port = port;
        self
    }

    pub fn build(self) -> ProxyCapturedChannel<tonic::transport::Channel> {
        ProxyCapturedChannel { inner: self.inner, scheme: self.scheme, host: self.host, port: self.port, log: capture_log_clone() }
    }
}


// ===== Unit tests for client-side capture (feature-gated) =====
#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use event_log::{EventRecord, JsonlEventLog};
    use http::Request;
    use serde_json::Value as JsonValue;
    use tonic::body::BoxBody;
    use tower::{service_fn, Service};

    fn run_captured_call_with_headers(headers: &[(&str, &str)], log: &JsonlEventLog) {
        std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
        super::test_set_capture_log(log.clone());

        let inner = service_fn(|_req: Request<BoxBody>| async move {
            Ok::<http::Response<tonic::transport::Body>, ()>(http::Response::new(
                tonic::transport::Body::empty(),
            ))
        });
        let mut svc = super::wrap_service(inner);

        let mut req = Request::builder()
            .uri("/orca.v1.Orchestrator/StartRun")
            .body(BoxBody::default())
            .unwrap();
        for (k, v) in headers {
            req.headers_mut().insert(*k, http::HeaderValue::from_str(v).unwrap());
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let _ = svc.call(req).await;
        });
    }

    fn read_log_events(log: &JsonlEventLog) -> Vec<EventRecord<JsonValue>> {
        log.read_range(0, u64::MAX).unwrap()
    }

    #[test]
    fn client_emits_external_io_started_and_finished_with_correlation() {
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client.jsonl")).unwrap();

        run_captured_call_with_headers(&[("authorization", "Bearer token")], &log);
        let recs = read_log_events(&log);

        let started = recs
            .iter()
            .rev()
            .find(|r| r.payload.get("event").and_then(|v| v.as_str()) == Some("external_io_started"))
            .expect("expected ExternalIoStarted");
        let finished = recs
            .iter()
            .rev()
            .find(|r| r.payload.get("event").and_then(|v| v.as_str()) == Some("external_io_finished"))
            .expect("expected ExternalIoFinished");

        let dir_s = started.payload.get("direction").and_then(|v| v.as_str()).unwrap();
        assert_eq!(dir_s, "client");
        let rid_s = started.payload.get("request_id").and_then(|v| v.as_str()).unwrap();
        let rid_f = finished.payload.get("request_id").and_then(|v| v.as_str()).unwrap();
        assert_eq!(rid_s, rid_f, "request_id must correlate started/finished");
    }

    #[test]
    fn client_redaction_only_when_sensitive_headers_present() {
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client2.jsonl")).unwrap();

        run_captured_call_with_headers(&[], &log);
        run_captured_call_with_headers(&[("authorization", "Bearer token")], &log);

        let recs = read_log_events(&log);
        let mut started_events: Vec<&EventRecord<JsonValue>> = recs
            .iter()
            .filter(|r| r.payload.get("event").and_then(|v| v.as_str()) == Some("external_io_started"))
            .collect();
        assert!(started_events.len() >= 2);
        let first = started_events.remove(0);
        let second = started_events.remove(0);
        let h1 = first.payload.get("headers").unwrap();
        let h2 = second.payload.get("headers").unwrap();
        let h1_str = h1.to_string();
        let h2_str = h2.to_string();
        assert!(h1_str == "{}" || h2_str == "{}", "one headers map should be empty");
        assert!(h1_str.contains("authorization") || h2_str.contains("authorization"));
    }

    #[cfg(feature = "otel")]
    #[test]
    fn metrics_stubs_feature_gated_and_emitted_under_otel() {
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client3.jsonl")).unwrap();

        run_captured_call_with_headers(&[], &log);
        let recs = read_log_events(&log);
        let has_metric = recs.iter().any(|r| {
            r.payload.get("metric").and_then(|v| v.as_str()) == Some("proxy.capture.duration_ms")
        });
        assert!(has_metric, "expected duration metric to be emitted under otel feature");
    }
}
