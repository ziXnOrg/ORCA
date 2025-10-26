use event_log::JsonlEventLog;
use orchestrator::orca_v1::orchestrator_server::Orchestrator;
use orchestrator::{orca_v1::*, OrchestratorService};
use tonic::Request;

#[tokio::test]
async fn warn_and_exceed_budget() {
    let dir = tempfile::tempdir().unwrap();
    let log = JsonlEventLog::open(dir.path().join("b.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);

    // Load a permissive policy to accommodate fail-closed baseline in tests
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(&policy_path, "rules: []\n").unwrap();
    svc.load_policy_from_path(&policy_path).unwrap();

    // Start run with very small token budget (1 token max)
    let start = StartRunRequest {
        workflow_id: "run1".into(),
        initial_task: None,
        budget: Some(Budget { max_tokens: 1, max_cost_micros: 0 }),
    };
    svc.start_run(Request::new(start)).await.unwrap();

    // Submit first task: should pass (tokens=1)
    let env1 = Envelope {
        id: "t1".into(),
        parent_id: "".into(),
        trace_id: "tr".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: "{}".into(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: 0,
        usage: None,
    };
    let r1 = svc
        .submit_task(Request::new(SubmitTaskRequest { run_id: "run1".into(), task: Some(env1) }))
        .await;
    assert!(r1.is_ok());

    // Submit second task: should exceed (tokens would be 2 > 1)
    let env2 = Envelope {
        id: "t2".into(),
        parent_id: "".into(),
        trace_id: "tr".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: "{}".into(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: 0,
        usage: None,
    };
    let r2 = svc
        .submit_task(Request::new(SubmitTaskRequest { run_id: "run1".into(), task: Some(env2) }))
        .await;
    assert!(r2.is_err());
    let status = r2.err().unwrap();
    assert_eq!(status.code(), tonic::Code::ResourceExhausted);
}

#[tokio::test]
async fn isolation_between_runs() {
    let dir = tempfile::tempdir().unwrap();
    let log = JsonlEventLog::open(dir.path().join("c.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);

    // Load a permissive policy to accommodate fail-closed baseline in tests
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(&policy_path, "rules: []\n").unwrap();
    svc.load_policy_from_path(&policy_path).unwrap();

    let start1 = StartRunRequest {
        workflow_id: "rA".into(),
        initial_task: None,
        budget: Some(Budget { max_tokens: 1, max_cost_micros: 0 }),
    };
    let start2 = StartRunRequest {
        workflow_id: "rB".into(),
        initial_task: None,
        budget: Some(Budget { max_tokens: 1, max_cost_micros: 0 }),
    };
    svc.start_run(Request::new(start1)).await.unwrap();
    svc.start_run(Request::new(start2)).await.unwrap();

    let env = Envelope {
        id: "t".into(),
        parent_id: "".into(),
        trace_id: "tr".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: "{}".into(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: 0,
        usage: None,
    };
    // Consume budget in rA
    assert!(svc
        .submit_task(Request::new(SubmitTaskRequest {
            run_id: "rA".into(),
            task: Some(env.clone())
        }))
        .await
        .is_ok());
    // rB should still be allowed
    assert!(svc
        .submit_task(Request::new(SubmitTaskRequest { run_id: "rB".into(), task: Some(env) }))
        .await
        .is_ok());
}
