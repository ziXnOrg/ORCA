use orchestrator::orca_v1::{orchestrator_server::Orchestrator, SubmitTaskRequest};
use orchestrator::OrchestratorService;
use serde_json::json;

#[tokio::test]
async fn audit_reason_redacts_pii_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let log = event_log::JsonlEventLog::open(dir.path().join("audit_pii.jsonl")).unwrap();
    let log_read = log.clone();
    let svc = OrchestratorService::new(log);

    // Policy with modify on pii_detect
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(
        &policy_path,
        r#"rules:
  - name: Redact-PII-Patterns
    when: pii_detect
    action: modify
    message: "redacted: 123-45-6789"
"#,
    )
    .unwrap();
    svc.load_policy_from_path(&policy_path).unwrap();

    // Envelope contains an SSN-like pattern that should be redacted in payload; audit reason must not leak PII
    let payload = json!({"text":"My SSN is 123-45-6789"});
    let env = orchestrator::orca_v1::Envelope {
        id: "m3".into(),
        parent_id: "".into(),
        trace_id: "t3".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: serde_json::to_string(&payload).unwrap(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: orchestrator::clock::process_clock().now_ms(),
        usage: None,
    };
    let _ = svc
        .submit_task(tonic::Request::new(SubmitTaskRequest {
            run_id: "r3".into(),
            task: Some(env),
        }))
        .await
        .unwrap();

    // Verify audit has no raw PII in reason
    let recs: Vec<event_log::EventRecord<serde_json::Value>> =
        log_read.read_range(0, u64::MAX).unwrap();
    let audits: Vec<_> = recs
        .into_iter()
        .filter(|r| r.payload.get("event").and_then(|v| v.as_str()) == Some("policy_audit"))
        .collect();
    assert!(!audits.is_empty(), "expected audit event for modify");
    let p = &audits[audits.len() - 1].payload;
    let reason = p.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!reason.contains("123-45-6789"), "audit reason must be redacted");
}
