use criterion::{criterion_group, criterion_main, Criterion};
use event_log::JsonlEventLog;
use orchestrator::orca_v1::orchestrator_client::OrchestratorClient;
use orchestrator::orca_v1::*;
use orchestrator::OrchestratorService;
use std::net::SocketAddr;
use tokio::runtime::Runtime;
use tonic::transport::{Channel, Server};

fn start_server(rt: &Runtime, capture_on: bool) -> (SocketAddr, tempfile::TempDir) {
    // Toggle server-side capture via env (read at request time)
    if capture_on {
        std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
    } else {
        std::env::remove_var("ORCA_CAPTURE_EXTERNAL_IO");
    }

    let dir = tempfile::tempdir().unwrap();
    let log = JsonlEventLog::open(dir.path().join("orc.jsonl")).unwrap();
    let svc = OrchestratorService::new(log);
    let svc = orchestrator::orca_v1::orchestrator_server::OrchestratorServer::new(svc);

    // Bind ephemeral port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server
    let svc_clone = svc;
    rt.spawn(async move {
        Server::builder().add_service(svc_clone).serve(addr).await.unwrap();
    });

    (addr, dir)
}

fn bench_capture_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (addr, _dir) = start_server(&rt, false);
    let endpoint = format!("http://{}", addr);

    // Build base channel shared across benches
    let channel =
        rt.block_on(async { Channel::from_shared(endpoint).unwrap().connect().await.unwrap() });

    // Bench OFF (capture disabled)
    std::env::remove_var("ORCA_CAPTURE_EXTERNAL_IO");
    c.bench_function("grpc_round_trip_off", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let fut = async {
                    let mut client = OrchestratorClient::new(channel.clone());
                    let _ = client
                        .start_run(StartRunRequest {
                            workflow_id: "wf".into(),
                            initial_task: None,
                            budget: None,
                        })
                        .await
                        .unwrap();
                };
                let start = std::time::Instant::now();
                rt.block_on(fut);
                total += start.elapsed();
            }
            total
        })
    });

    // Bench ON (server-side capture enabled)
    std::env::set_var("ORCA_CAPTURE_EXTERNAL_IO", "1");
    c.bench_function("grpc_round_trip_on", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let fut = async {
                    let mut client = OrchestratorClient::new(channel.clone());
                    let _ = client
                        .start_run(StartRunRequest {
                            workflow_id: "wf".into(),
                            initial_task: None,
                            budget: None,
                        })
                        .await
                        .unwrap();
                };
                let start = std::time::Instant::now();
                rt.block_on(fut);
                total += start.elapsed();
            }
            total
        })
    });
}

criterion_group!(benches, bench_capture_overhead);
criterion_main!(benches);
