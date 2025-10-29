use event_log::JsonlEventLog;
use orchestrator::orca_v1::orchestrator_server::Orchestrator;
use orchestrator::orca_v1::*;
use orchestrator::OrchestratorService;
use serde_json::json;

#[tokio::test]
async fn orchestrator_emits_attachments_metadata_red() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attachments.jsonl");
    let log = JsonlEventLog::open(&path).unwrap();
    let svc = OrchestratorService::new(log.clone());

    // Simulate a task that references a blob by digest. The orchestrator should emit
    // attachments metadata (digest/size/mime) and NOT embed raw payloads.
    let payload = json!({
        "kind": "agent_task",
        "blob_ref": {
            "digest_sha256": "00e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0deadbeef",
            "size_bytes": 1024u64,
            "mime": "text/plain"
        },
        // Raw bytes would have been here in naive systems; we require metadata-only in WAL
    });

    let env = Envelope {
        id: "m1".into(),
        parent_id: "".into(),
        trace_id: "t1".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: payload.to_string(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: 1,
        usage: None,
    };

    let _ = svc
        .submit_task(tonic::Request::new(SubmitTaskRequest {
            run_id: "wf1".into(),
            task: Some(env),
        }))
        .await;

    // Read WAL and assert that an attachments array is present in the emitted record
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(file.contains("\"attachments\""), "expected attachments array in WAL record");
}
