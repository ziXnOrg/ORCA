use orchestrator::{orca_v1::*, OrchestratorService};
use event_log::JsonlEventLog;
use tonic::Request;

#[tokio::test]
async fn warn_and_exceed_budget() {
    let dir = tempfile::tempdir().unwrap();
    let log = JsonlEventLog::open(dir.path().join("b.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);

    // Start run with very small token budget
    let start = StartRunRequest { workflow_id: "run1".into(), initial_task: None, budget: Some(Budget{ max_tokens: 2, max_cost_micros: 0 }) };
    svc.start_run(Request::new(start)).await.unwrap();

    // Submit two tasks: first should pass, second should exceed
    let env = Envelope { id: "t1".into(), parent_id: "".into(), trace_id: "tr".into(), agent: "A".into(), kind: "agent_task".into(), payload_json: "{}".into(), timeout_ms: 0, protocol_version: 1, ts_ms: 0, usage: None };
    let r1 = svc.submit_task(Request::new(SubmitTaskRequest { run_id: "run1".into(), task: Some(env.clone()) })).await;
    assert!(r1.is_ok());
    let r2 = svc.submit_task(Request::new(SubmitTaskRequest { run_id: "run1".into(), task: Some(env) })).await;
    assert!(r2.is_err());
    let status = r2.err().unwrap();
    assert_eq!(status.code(), tonic::Code::ResourceExhausted);
}

#[tokio::test]
async fn isolation_between_runs() {
    let dir = tempfile::tempdir().unwrap();
    let log = JsonlEventLog::open(dir.path().join("c.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);

    let start1 = StartRunRequest { workflow_id: "rA".into(), initial_task: None, budget: Some(Budget{ max_tokens: 1, max_cost_micros: 0 }) };
    let start2 = StartRunRequest { workflow_id: "rB".into(), initial_task: None, budget: Some(Budget{ max_tokens: 1, max_cost_micros: 0 }) };
    svc.start_run(Request::new(start1)).await.unwrap();
    svc.start_run(Request::new(start2)).await.unwrap();

    let env = Envelope { id: "t".into(), parent_id: "".into(), trace_id: "tr".into(), agent: "A".into(), kind: "agent_task".into(), payload_json: "{}".into(), timeout_ms: 0, protocol_version: 1, ts_ms: 0, usage: None };
    // Consume budget in rA
    assert!(svc.submit_task(Request::new(SubmitTaskRequest { run_id: "rA".into(), task: Some(env.clone()) })).await.is_ok());
    // rB should still be allowed
    assert!(svc.submit_task(Request::new(SubmitTaskRequest { run_id: "rB".into(), task: Some(env) })).await.is_ok());
}


