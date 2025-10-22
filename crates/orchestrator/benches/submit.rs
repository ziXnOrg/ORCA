use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use event_log::JsonlEventLog;
use orchestrator::orca_v1::orchestrator_server::Orchestrator;
use orchestrator::{orca_v1::*, OrchestratorService};
use serde_json::json;
use tokio::runtime::Runtime;

fn bench_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    c.bench_function("submit_task_stub", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let log = JsonlEventLog::open(dir.path().join("orc.jsonl")).unwrap();
                let svc = OrchestratorService::new(log);
                (svc, dir)
            },
            |(svc, _dir)| {
                rt.block_on(async {
                    let env = Envelope {
                        id: "m".into(),
                        parent_id: "".into(),
                        trace_id: "t".into(),
                        agent: "A".into(),
                        kind: "agent_task".into(),
                        payload_json: json!({}).to_string(),
                        timeout_ms: 0,
                        protocol_version: 1,
                        ts_ms: 1,
                        usage: None,
                    };
                    let _ = svc
                        .submit_task(tonic::Request::new(SubmitTaskRequest {
                            run_id: "wf".into(),
                            task: Some(env),
                        }))
                        .await;
                })
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_submit);
criterion_main!(benches);
