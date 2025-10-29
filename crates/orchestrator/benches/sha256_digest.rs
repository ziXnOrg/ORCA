use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orchestrator::proxy::sha256_hex;
use sha2::{Digest, Sha256};

fn make_payload(size: usize) -> Vec<u8> {
    let mut v = vec![0u8; size];
    for (i, b) in v.iter_mut().enumerate() {
        *b = (i as u32 % 251) as u8;
    }
    v
}

fn bench_sha256_builtin(c: &mut Criterion) {
    let sizes = [1 * 1024, 64 * 1024, 1 * 1024 * 1024, 10 * 1024 * 1024];
    let mut group = c.benchmark_group("sha256_hex_builtin");
    for &sz in &sizes {
        let data = make_payload(sz);
        group.bench_with_input(BenchmarkId::from_parameter(sz), &data, |b, d| {
            b.iter(|| {
                let _ = black_box(sha256_hex(d));
            })
        });
    }
    group.finish();
}

fn bench_sha256_chunk_sizes(c: &mut Criterion) {
    let sizes = [1 * 1024, 64 * 1024, 1 * 1024 * 1024, 10 * 1024 * 1024];
    let chunks = [32 * 1024, 64 * 1024, 128 * 1024, 256 * 1024];
    let mut group = c.benchmark_group("sha256_chunk_sizes");
    for &sz in &sizes {
        let data = make_payload(sz);
        for &chunk in &chunks {
            group.bench_with_input(BenchmarkId::new(sz.to_string(), chunk), &data, |b, d| {
                b.iter(|| {
                    // manual chunking
                    let mut hasher = Sha256::new();
                    for ch in d.chunks(chunk) {
                        hasher.update(ch);
                    }
                    let _ = black_box(hex::encode(hasher.finalize()));
                })
            });
        }
    }
    group.finish();
}

criterion_group!(sha256_digest, bench_sha256_builtin, bench_sha256_chunk_sizes);
criterion_main!(sha256_digest);
