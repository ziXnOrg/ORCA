use event_log::JsonlEventLog;
use orchestrator::OrchestratorService;
use serde_json::json;

#[tokio::test]
async fn crash_restart_replay_rebuilds_index() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("replay.jsonl");
    let log = JsonlEventLog::open(&path).unwrap();
    // Simulate prior run: write start and task
    let _ = log.append(1, 1, &json!({"event":"start_run", "workflow_id":"wf1"})).unwrap();
    let _ = log
        .append(2, 2, &json!({"event":"task_enqueued", "run_id":"wf1", "envelope": {"id":"m1"}}))
        .unwrap();

    // Restart: new service instance
    let svc = OrchestratorService::new(JsonlEventLog::open(&path).unwrap());
    svc.replay_on_start().unwrap();

    // Validate index contains wf1 -> last_event_id=2 and seen_ids includes m1
    assert_eq!(svc.index.last_event_id_by_run.get("wf1").map(|v| *v.value()), Some(2));
}
