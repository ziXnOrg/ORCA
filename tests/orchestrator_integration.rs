use orchestrator::orca_v1::{orchestrator_client::OrchestratorClient, *};
use orchestrator::OrchestratorService;
use event_log::JsonlEventLog;
use tokio::net::TcpListener;
use tonic::transport::Server;
use serde_json::json;

async fn spawn_server() -> (String, tokio::task::JoinHandle<()>) {
    let dir = tempfile::tempdir().unwrap();
    let log = JsonlEventLog::open(dir.path().join("it.jsonl")).unwrap();
    let svc = OrchestratorService::new(log).into_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let h = tokio::spawn(async move {
        Server::builder().add_service(svc).serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener)).await.unwrap();
    });
    (format!("http://{}", addr), h)
}

#[tokio::test]
async fn happy_path_start_submit() {
    let (addr, _h) = spawn_server().await;
    let mut client = OrchestratorClient::connect(addr).await.unwrap();
    let env = Envelope { id: "t1".into(), parent_id: "".into(), trace_id: "tr".into(), agent: "A".into(), kind: "agent_task".into(), payload_json: json!({"x":1}).to_string(), timeout_ms: 0, protocol_version: 1, ts_ms: orca_core::ids::now_ms(), usage: None };
    let sr = client.start_run(StartRunRequest { workflow_id: "wf1".into(), initial_task: Some(env), budget: None }).await.unwrap().into_inner();
    assert_eq!(sr.run_id, "wf1");
    let env2 = Envelope { id: "t2".into(), parent_id: "".into(), trace_id: "tr".into(), agent: "A".into(), kind: "agent_task".into(), payload_json: "{}".into(), timeout_ms: 0, protocol_version: 1, ts_ms: orca_core::ids::now_ms(), usage: None };
    let ok = client.submit_task(SubmitTaskRequest { run_id: "wf1".into(), task: Some(env2) }).await.unwrap().into_inner();
    assert!(ok.accepted);
}

#[tokio::test]
async fn ttl_timeout_is_rejected() {
    let (addr, _h) = spawn_server().await;
    let mut client = OrchestratorClient::connect(addr).await.unwrap();
    let env = Envelope { id: "t3".into(), parent_id: "".into(), trace_id: "tr".into(), agent: "A".into(), kind: "agent_task".into(), payload_json: "{}".into(), timeout_ms: 1, protocol_version: 1, ts_ms: orca_core::ids::now_ms().saturating_sub(10_000), usage: None };
    let res = client.submit_task(SubmitTaskRequest { run_id: "wf1".into(), task: Some(env) }).await;
    assert!(res.is_err());
}
