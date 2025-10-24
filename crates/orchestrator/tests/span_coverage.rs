use std::sync::{Arc, Mutex};

use orchestrator::orca_v1::{orchestrator_server::Orchestrator, StartRunRequest, SubmitTaskRequest};
use orchestrator::{orca_v1, OrchestratorService};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer, Registry};

struct RecordingLayer {
    spans: Arc<Mutex<Vec<String>>>,
}
impl<S> Layer<S> for RecordingLayer
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &tracing::span::Id, ctx: Context<'_, S>) {
        let meta = ctx.metadata(id).unwrap_or_else(|| attrs.metadata());
        let name = meta.name().to_string();
        self.spans.lock().unwrap().push(name);
    }
}

#[tokio::test]
async fn spans_present_for_key_paths() {
    // Install recording subscriber
    let recorded = Arc::new(Mutex::new(Vec::<String>::new()));
    let layer = RecordingLayer { spans: recorded.clone() };
    let subscriber = Registry::default().with(layer);
    let _guard = tracing::subscriber::set_default(subscriber);

    // Set up service and WAL
    let dir = tempfile::tempdir().unwrap();
    let log = event_log::JsonlEventLog::open(dir.path().join("x.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);

    // start_run path
    let env = orca_v1::Envelope {
        id: "e0".into(),
        parent_id: "".into(),
        trace_id: "t".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: "{}".into(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: orca_core::ids::now_ms(),
        usage: None,
    };
    let _ = svc
        .start_run(tonic::Request::new(StartRunRequest { workflow_id: "wf".into(), initial_task: Some(env), budget: None }))
        .await
        .unwrap();

    // submit_task path
    let env2 = orca_v1::Envelope {
        id: "e1".into(),
        parent_id: "".into(),
        trace_id: "t".into(),
        agent: "A".into(),
        kind: "agent_task".into(),
        payload_json: "{}".into(),
        timeout_ms: 0,
        protocol_version: 1,
        ts_ms: orca_core::ids::now_ms(),
        usage: None,
    };
    let _ = svc
        .submit_task(tonic::Request::new(SubmitTaskRequest { run_id: "wf".into(), task: Some(env2) }))
        .await
        .unwrap();

    let names = recorded.lock().unwrap().clone();
    // Check that our key span names were created at least once
    assert!(names.iter().any(|n| n.contains("agent.policy.check")), "missing policy span");
    assert!(names.iter().any(|n| n.contains("wal.append")), "missing wal span");
    assert!(names.iter().any(|n| n.contains("agent.budget.check")), "missing budget span");
}
