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

#[cfg(feature = "capture")]
#[derive(serde::Serialize)]
struct ExternalIoStarted {
    event: &'static str,
    system: &'static str,
    direction: &'static str,
    scheme: String,
    host: String,
    port: u16,
    method: String,
    request_id: String,
    body_digest_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<JsonMap<String, JsonValue>>,
}

#[cfg(feature = "capture")]
#[derive(serde::Serialize)]
struct ExternalIoFinished {
    event: &'static str,
    request_id: String,
    status: &'static str,
    duration_ms: u64,
}

// ===== Client-side capture layer (wired behind `capture` feature) =====
use http::{Request, Response};
use std::task::{Context, Poll};
#[cfg(feature = "capture")]
use std::{future::Future, pin::Pin};
use tonic::body::BoxBody;
use tower::{Layer, Service};

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

#[cfg_attr(not(feature = "capture"), allow(dead_code))]
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
    S::Error: From<tonic::Status>,
{
    type Response = Response<tonic::transport::Body>;
    type Error = S::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Response<tonic::transport::Body>, S::Error>> + Send>>;

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
            let headers_opt = redacted_headers_from_http(req.headers());
            // Build started payload (typed) and include headers only when non-empty (Opt 3 + Opt 6).
            let started = ExternalIoStarted {
                event: "external_io_started",
                system: "grpc",
                direction: "client",
                scheme: self.scheme.clone(),
                host: self.host.clone(),
                port: self.port,
                method: method_path,
                request_id: rid.clone(),
                body_digest_sha256: sha256_hex(&[]),
                headers: headers_opt,
            };
            let __append_res = logc.append(orca_core::ids::next_monotonic_id(), t0, &started);
            let mut __append_failed = __append_res.is_err();
            #[cfg(test)]
            {
                __append_failed = __append_failed || fail_inject_enabled();
            }
            if __append_failed && !bypass_to_direct() {
                return Box::pin(async move {
                    Err::<Response<tonic::transport::Body>, S::Error>(
                        tonic::Status::failed_precondition("client capture WAL append failed").into(),
                    )
                });
            }
        }

        let fut = self.inner.call(req);
        Box::pin(async move {
            let res = fut.await;
            if let Some(logc) = log.clone() {
                let t1 = crate::clock::process_clock().now_ms();
                let status = if res.is_ok() { "ok" } else { "error" };
                let finished = ExternalIoFinished {
                    event: "external_io_finished",
                    request_id: rid,
                    status,
                    duration_ms: t1.saturating_sub(t0),
                };
                let __append_res2 = logc.append(orca_core::ids::next_monotonic_id(), t1, &finished);
                let mut __append_failed2 = __append_res2.is_err();
                #[cfg(test)]
                {
                    __append_failed2 = __append_failed2 || fail_inject_enabled();
                }
                if __append_failed2 && !bypass_to_direct() {
                    return Err::<Response<tonic::transport::Body>, S::Error>(
                        tonic::Status::failed_precondition("client capture WAL append failed").into(),
                    );
                }
                #[cfg(feature = "otel")]
                {
                    // WAL metric emission remains for auditability; OTel metrics to be added in a follow-up optimization.
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
pub(crate) fn wrap_service<S>(inner: S) -> ProxyCapturedChannel<S> {
    ProxyCaptureLayer.layer(inner)
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
        ProxyCapturedChannel {
            inner: self.inner,
            scheme: self.scheme,
            host: self.host,
            port: self.port,
            log: capture_log_clone(),
        }
    }
}

// ===== Unit tests for client-side capture (feature-gated) =====
#[cfg(all(test, feature = "capture"))]
mod tests {

    use event_log::{EventRecord, JsonlEventLog};
    use http::Request;
    use serde_json::Value as JsonValue;
    use std::sync::{Mutex, OnceLock};
    use tonic::body::BoxBody;
    use tower::{service_fn, Service};

    static TEST_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn run_captured_call_with_headers(headers: &[(&str, &str)], log: &JsonlEventLog) {
        std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
        super::test_set_capture_log(log.clone());

        let inner = service_fn(|_req: Request<BoxBody>| async move {
            Ok::<http::Response<tonic::transport::Body>, tonic::Status>(http::Response::new(
                tonic::transport::Body::empty(),
            ))
        });
        let mut svc = super::wrap_service(inner);

        let mut req = Request::builder()
            .uri("/orca.v1.Orchestrator/StartRun")
            .body(BoxBody::default())
            .unwrap();
        for (k, v) in headers {
            let key = http::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            let val = http::HeaderValue::from_bytes(v.as_bytes()).unwrap();
            req.headers_mut().insert(key, val);
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
        let _g = serial_guard();
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client.jsonl")).unwrap();

        run_captured_call_with_headers(&[("authorization", "Bearer token")], &log);
        // no-op read removed (was for debug)

        let recs = read_log_events(&log);

        let started = recs
            .iter()
            .rev()
            .find(|r| {
                r.payload.get("event").and_then(|v| v.as_str()) == Some("external_io_started")
            })
            .expect("expected ExternalIoStarted");
        let finished = recs
            .iter()
            .rev()
            .find(|r| {
                r.payload.get("event").and_then(|v| v.as_str()) == Some("external_io_finished")
            })
            .expect("expected ExternalIoFinished");

        let dir_s = started.payload.get("direction").and_then(|v| v.as_str()).unwrap();
        assert_eq!(dir_s, "client");
        let rid_s = started.payload.get("request_id").and_then(|v| v.as_str()).unwrap();
        let rid_f = finished.payload.get("request_id").and_then(|v| v.as_str()).unwrap();
        assert_eq!(rid_s, rid_f, "request_id must correlate started/finished");
    }

    #[test]
    fn client_redaction_only_when_sensitive_headers_present() {
        let _g = serial_guard();
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client2.jsonl")).unwrap();

        run_captured_call_with_headers(&[], &log);
        run_captured_call_with_headers(&[("authorization", "Bearer token")], &log);

        // no-op read removed (was for debug)

        let recs = read_log_events(&log);
        let mut started_events: Vec<&EventRecord<JsonValue>> = recs
            .iter()
            .filter(|r| {
                r.payload.get("event").and_then(|v| v.as_str()) == Some("external_io_started")
            })
            .collect();
        assert!(started_events.len() >= 2);
        let first = started_events.remove(0);
        let second = started_events.remove(0);
        // When no sensitive headers are present, the headers field should be absent.
        assert!(
            first.payload.get("headers").is_none(),
            "headers should be absent when no sensitive headers present"
        );
        // When sensitive headers are present, headers should include redacted entries.
        let h2 =
            second.payload.get("headers").expect("headers should be present for sensitive headers");
        let h2_str = h2.to_string();
        assert!(
            h2_str.contains("authorization"),
            "expected authorization to be redacted in headers"
        );
    }

    #[cfg(feature = "otel")]
    #[test]
    fn metrics_stubs_feature_gated_and_emitted_under_otel() {
        let _g = serial_guard();
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client3.jsonl")).unwrap();

        run_captured_call_with_headers(&[], &log);
        let recs = read_log_events(&log);
        let has_metric = recs.iter().any(|r| {
            r.payload.get("metric").and_then(|v| v.as_str()) == Some("proxy.capture.duration_ms")
        });
        assert!(has_metric, "expected duration metric to be emitted under otel feature");
    }

    #[test]
    fn client_denies_request_on_wal_append_failure_by_default() {
        let _g = serial_guard();
        std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
        std::env::remove_var("ORCA_BYPASS_TO_DIRECT");
        std::env::set_var("ORCA_CAPTURE_FAIL_INJECT", "1");
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client_fail.jsonl")).unwrap();
        super::test_set_capture_log(log.clone());

        let inner = service_fn(|_req: Request<BoxBody>| async move {
            Ok::<http::Response<tonic::transport::Body>, tonic::Status>(http::Response::new(
                tonic::transport::Body::empty(),
            ))
        });
        let mut svc = super::wrap_service(inner);
        let req = Request::builder()
            .uri("/orca.v1.Orchestrator/StartRun")
            .body(BoxBody::default())
            .unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(async move { svc.call(req).await });
        assert!(res.is_err(), "expected call to be denied on WAL append failure by default");

        // cleanup
        std::env::remove_var("ORCA_CAPTURE_FAIL_INJECT");
    }

    #[test]
    fn client_allows_request_on_wal_append_failure_when_bypass_enabled() {
        let _g = serial_guard();
        std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
        std::env::set_var("ORCA_BYPASS_TO_DIRECT", "1");
        std::env::set_var("ORCA_CAPTURE_FAIL_INJECT", "1");
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("client_bypass.jsonl")).unwrap();
        super::test_set_capture_log(log.clone());

        let inner = service_fn(|_req: Request<BoxBody>| async move {
            Ok::<http::Response<tonic::transport::Body>, tonic::Status>(http::Response::new(
                tonic::transport::Body::empty(),
            ))
        });
        let mut svc = super::wrap_service(inner);
        let req = Request::builder()
            .uri("/orca.v1.Orchestrator/StartRun")
            .body(BoxBody::default())
            .unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(async move { svc.call(req).await });
        assert!(res.is_ok(), "expected call to proceed when bypass enabled despite WAL append failure");

        // cleanup
        std::env::remove_var("ORCA_BYPASS_TO_DIRECT");
        std::env::remove_var("ORCA_CAPTURE_FAIL_INJECT");
    }
}

