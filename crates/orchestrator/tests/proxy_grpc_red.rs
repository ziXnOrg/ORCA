//! RED tests for HTTP/gRPC external I/O capture (T-6a-E1-PROXY-11)
//! These tests intentionally fail until the capture skeleton is implemented.

use event_log::{EventRecord, JsonlEventLog};
use orchestrator::orca_v1::{orchestrator_client::OrchestratorClient, *};
use orchestrator::OrchestratorService;
use serde_json::Value as JsonValue;
use tokio::net::TcpListener;
use tonic::{metadata::MetadataValue, transport::Server, Status};

async fn spawn_server() -> (String, tokio::task::JoinHandle<()>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("it.jsonl");
    let log = JsonlEventLog::open(&log_path).unwrap();

    let svc_impl = OrchestratorService::new(log);
    // Load a permissive policy to accommodate fail-closed baseline in tests
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(&policy_path, "rules: []\n").unwrap();
    svc_impl.load_policy_from_path(&policy_path).unwrap();

    let svc = svc_impl.into_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let h = tokio::spawn(async move {
        let stream = futures_util::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.ok()?.0, listener))
        })
        .filter_map(|s| async move { Some(Ok::<_, std::io::Error>(s)) });
        Server::builder().add_service(svc).serve_with_incoming(stream).await.unwrap();
    });
    (format!("http://{}", addr), h, dir)
}

fn test_env_envelope(id: &str) -> Envelope {
    Envelope {
        id: id.into(),
        parent_id: "".into(),
        trace_id: "t".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: "{\"x\":1}".into(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: orca_core::ids::now_ms(),
        usage: None,
    }
}

#[tokio::test]
async fn wal_stubs_emitted_for_grpc_capture_red() {
    // Enable capture (flag will be wired in GREEN)
    std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");

    let (addr, _h, dir) = spawn_server().await;
    let mut client = OrchestratorClient::connect(addr).await.unwrap();

    // Include an Authorization header to validate redaction later
    let mut md = tonic::metadata::MetadataMap::new();
    md.insert(
        "authorization",
        MetadataValue::try_from("Bearer secret-token").unwrap(),
    );

    // StartRun + SubmitTask to generate at least some WAL traffic
    let _ = client
        .start_run(StartRunRequest { workflow_id: "wf1".into(), initial_task: Some(test_env_envelope("t1")), budget: None })
        .await
        .unwrap()
        .into_inner();

    let _ = client
        .submit_task(SubmitTaskRequest { run_id: "wf1".into(), task: Some(test_env_envelope("t2")) })
        .await
        .unwrap()
        .into_inner();

    // Read WAL and assert external io capture stubs exist
    let log = JsonlEventLog::open(dir.path().join("it.jsonl")).unwrap();
    let recs: Vec<EventRecord<JsonValue>> = log.read_range(0, u64::MAX).unwrap();

    let started: Vec<_> = recs
        .iter()
        .filter_map(|r| r.payload.get("event").and_then(|v| v.as_str()).filter(|e| *e == "external_io_started").map(|_| &r.payload))
        .collect();
    assert!(
        !started.is_empty(),
        "expected at least one external_io_started event in WAL (RED)"
    );

    let finished: Vec<_> = recs
        .iter()
        .filter_map(|r| r.payload.get("event").and_then(|v| v.as_str()).filter(|e| *e == "external_io_finished").map(|_| &r.payload))
        .collect();
    assert!(
        !finished.is_empty(),
        "expected at least one external_io_finished event in WAL (RED)"
    );

    // Correlate by request_id and check required fields (these structures will be implemented in GREEN)
    let req_id = started[0].get("request_id").and_then(|v| v.as_str()).expect("request_id field");
    let f = finished
        .into_iter()
        .find(|p| p.get("request_id").and_then(|v| v.as_str()) == Some(req_id))
        .expect("matching finished event with same request_id");

    // system, direction, scheme, host, port, method present
    for k in ["system", "direction", "scheme", "host", "port", "method"] {
        assert!(started[0].get(k).is_some(), "missing key {} in started", k);
    }
    // status + duration_ms present in finished
    assert!(f.get("status").is_some(), "missing status in finished");
    assert!(f.get("duration_ms").is_some(), "missing duration_ms in finished");
}

#[tokio::test]
async fn redaction_is_applied_red() {
    std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
    let (addr, _h, dir) = spawn_server().await;
    let mut client = OrchestratorClient::connect(addr).await.unwrap();

    // Call that should be captured; include sensitive header to test redaction
    let _ = client
        .start_run(StartRunRequest { workflow_id: "wf2".into(), initial_task: Some(test_env_envelope("t10")), budget: None })
        .await
        .unwrap();

    let log = JsonlEventLog::open(dir.path().join("it.jsonl")).unwrap();
    let recs: Vec<EventRecord<JsonValue>> = log.read_range(0, u64::MAX).unwrap();
    let started = recs
        .iter()
        .filter_map(|r| r.payload.get("event").and_then(|v| v.as_str()).filter(|e| *e == "external_io_started").map(|_| &r.payload))
        .next()
        .expect("expected an external_io_started event (RED)");

    // Expect header redaction and body digest instead of raw content
    let headers = started.get("headers").and_then(|v| v.as_object()).expect("headers object");
    assert_eq!(
        headers.get("authorization").and_then(|v| v.as_str()),
        Some("[REDACTED]")
    );
    assert!(started.get("body").is_none(), "raw body must not be recorded");
    assert!(
        started.get("body_digest_sha256").is_some(),
        "body_digest_sha256 must be present"
    );
}

#[tokio::test]
async fn fail_closed_on_capture_error_red() {
    // Inject a capture failure; by default requests should be denied (bypass only when explicitly enabled)
    std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
    std::env::set_var("ORCA_CAPTURE_FAIL_INJECT", "1");

    let (addr, _h, _dir) = spawn_server().await;
    let mut client = OrchestratorClient::connect(addr).await.unwrap();

    let res = client
        .submit_task(SubmitTaskRequest { run_id: "wf3".into(), task: Some(test_env_envelope("t20")) })
        .await;

    // RED: until implemented, this likely returns Ok; we expect Err to enforce fail-closed
    assert!(res.is_err(), "capture failure should deny the request (RED)");

    // And with bypass_to_direct=true it should proceed (GREEN will wire the flag)
    std::env::remove_var("ORCA_CAPTURE_FAIL_INJECT");
    std::env::set_var("ORCA_BYPASS_TO_DIRECT", "1");
    let ok = client
        .submit_task(SubmitTaskRequest { run_id: "wf3".into(), task: Some(test_env_envelope("t21")) })
        .await;
    assert!(ok.is_ok(), "bypass_to_direct=true should allow request to proceed (RED)");
}

#[tokio::test]
async fn perf_scaffolding_metrics_red() {
    std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
    let (addr, _h, dir) = spawn_server().await;
    let mut client = OrchestratorClient::connect(addr).await.unwrap();

    let t0 = std::time::Instant::now();
    let _ = client
        .submit_task(SubmitTaskRequest { run_id: "wf4".into(), task: Some(test_env_envelope("t30")) })
        .await
        .unwrap();
    let _elapsed_ms = t0.elapsed().as_millis() as u64;

    let log = JsonlEventLog::open(dir.path().join("it.jsonl")).unwrap();
    let recs: Vec<EventRecord<JsonValue>> = log.read_range(0, u64::MAX).unwrap();

    // Assert at least one timing metric related to proxy/capture was emitted (name TBD in GREEN)
    let metrics_count = recs
        .iter()
        .filter(|r| r
            .payload
            .get("metric")
            .and_then(|v| v.as_str())
            .map(|m| m.contains("proxy") || m.contains("capture")).unwrap_or(false))
        .count();
    assert!(metrics_count > 0, "expected capture-related timing metric in WAL or telemetry (RED)");
}

