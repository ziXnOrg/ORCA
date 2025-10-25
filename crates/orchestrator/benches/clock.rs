use criterion::{black_box, criterion_group, criterion_main, Criterion};
use orchestrator::clock::Clock;

fn bench_clock_now_ms(c: &mut Criterion) {
    let mut group = c.benchmark_group("clock_now_ms");
    // Small, fast functions: increase sample size for better resolution
    group.sample_size(1000);

    // VirtualClock baseline (no syscalls)
    let vclk = orchestrator::clock::VirtualClock::new(1);
    group.bench_function("virtual_clock_now_ms", |b| {
        b.iter(|| {
            // Fast path: single Mutex<u64> read
            black_box(vclk.now_ms())
        })
    });

    // SystemClock (wraps SystemTime)
    let sclk = orchestrator::clock::SystemClock::default();
    group.bench_function("system_clock_now_ms", |b| b.iter(|| black_box(sclk.now_ms())));

    // Direct SystemTime for reference
    group.bench_function("direct_systemtime_now", |b| {
        use std::time::{SystemTime, UNIX_EPOCH};
        b.iter(|| {
            let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            black_box(dur.as_millis() as u64)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_clock_now_ms);
criterion_main!(benches);
