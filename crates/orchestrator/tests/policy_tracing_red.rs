use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use orchestrator::orca_v1::{
    orchestrator_server::Orchestrator, StartRunRequest, SubmitTaskRequest,
};
use orchestrator::{orca_v1, OrchestratorService};
use tracing::field::{Field, Visit};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer, Registry};

#[derive(Default, Clone)]
struct RecordedSpan {
    name: String,
    fields: HashMap<String, String>,
}

struct FieldVisitor<'a> {
    map: &'a mut HashMap<String, String>,
}
impl<'a> Visit for FieldVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.map.insert(field.name().to_string(), format!("{:?}", value));
    }
}

struct RecordingLayer {
    spans: Arc<Mutex<Vec<RecordedSpan>>>,
}
impl<S> Layer<S> for RecordingLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        // Capture only policy spans
        let meta = ctx.metadata(id).unwrap_or_else(|| attrs.metadata());
        let name = meta.name().to_string();
        if name.contains("agent.policy.check") {
            let mut fields = HashMap::new();
            let mut vis = FieldVisitor { map: &mut fields };
            attrs.record(&mut vis);
            self.spans.lock().unwrap().push(RecordedSpan { name, fields });
        }
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            let meta = span.metadata();
            if meta.name().contains("agent.policy.check") {
                let mut guard = self.spans.lock().unwrap();
                if let Some(last) = guard.last_mut() {
                    let mut vis = FieldVisitor { map: &mut last.fields };
                    values.record(&mut vis);
                }
            }
        }
    }
}

#[tokio::test]
async fn policy_spans_include_required_attributes() {
    // Install recording subscriber
    let recorded = Arc::new(Mutex::new(Vec::<RecordedSpan>::new()));
    let layer = RecordingLayer { spans: recorded.clone() };
    let subscriber = Registry::default().with(layer);
    let _guard = tracing::subscriber::set_default(subscriber);

    // Service + WAL
    let dir = tempfile::tempdir().unwrap();
    let log = event_log::JsonlEventLog::open(dir.path().join("obs.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);

    // Permissive policy file to avoid fail-closed
    let policy_path = dir.path().join("policy.yaml");
    std::fs::write(&policy_path, "rules: []\n").unwrap();
    svc.load_policy_from_path(&policy_path).unwrap();

    // Start run (creates a policy span for pre_start_run)
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
        .start_run(tonic::Request::new(StartRunRequest {
            workflow_id: "wf".into(),
            initial_task: Some(env),
            budget: None,
        }))
        .await
        .unwrap();

    // Submit task (creates a policy span for pre_submit_task)
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
        .submit_task(tonic::Request::new(SubmitTaskRequest {
            run_id: "wf".into(),
            task: Some(env2),
        }))
        .await
        .unwrap();

    // Assert required low-cardinality attributes exist on at least one policy span
    let spans = recorded.lock().unwrap().clone();
    let mut ok = false;
    for s in spans.iter() {
        if s.name.contains("agent.policy.check") {
            let has_phase = s.fields.contains_key("phase");
            let has_decision_kind = s.fields.contains_key("decision_kind");
            if has_phase && has_decision_kind {
                ok = true;
                break;
            }
        }
    }
    assert!(ok, "expected agent.policy.check spans to include phase and decision_kind attributes");
}
