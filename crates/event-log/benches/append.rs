use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use event_log::JsonlEventLog;
use std::time::{SystemTime, UNIX_EPOCH};

fn ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn bench_append(c: &mut Criterion) {
    c.bench_function("append_jsonl", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("log.jsonl");
                let log = JsonlEventLog::open(&path).unwrap();
                (dir, log)
            },
            |(_dir, log)| {
                let id = 1;
                let _ = log.append(id, ts(), &"x");
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_append);
criterion_main!(benches);
