//! OTel-backed observer for Blob Store metrics (feature-gated via `otel`).
//! Provides counters for put/get bytes and cleanup count. Spans are best-effort.

use once_cell::sync::OnceCell;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Meter, Unit};
use opentelemetry::KeyValue;
use std::sync::atomic::{AtomicU64, Ordering};

use ::blob_store::{BlobSpan, BlobStoreObserver};

struct Instruments {
    put_bytes: Counter<u64>,
    get_bytes: Counter<u64>,
    cleanup_count: Counter<u64>,
}

static INSTR: OnceCell<Instruments> = OnceCell::new();
static INSTANCE: OnceCell<OtelBlobObserver> = OnceCell::new();

// Test-visible mirrors to assert increments in unit tests without exporter plumbing
static PUT_ACC: AtomicU64 = AtomicU64::new(0);
static GET_ACC: AtomicU64 = AtomicU64::new(0);
static CLEAN_ACC: AtomicU64 = AtomicU64::new(0);

fn ensure_instruments() -> &'static Instruments {
    INSTR.get_or_init(|| {
        // Use the global meter provider (may be a no-op if OTLP not initialized).
        let meter: Meter = global::meter("orca.blob");
        let put_bytes = meter
            .u64_counter("blob.put.bytes")
            .with_description("Plaintext bytes accepted by put()")
            .with_unit(Unit::new("By"))
            .init();
        let get_bytes = meter
            .u64_counter("blob.get.bytes")
            .with_description("Plaintext bytes returned by get()")
            .with_unit(Unit::new("By"))
            .init();
        let cleanup_count = meter
            .u64_counter("blob.cleanup.count")
            .with_description("Number of incomplete artifacts cleaned up")
            .init();
        Instruments { put_bytes, get_bytes, cleanup_count }
    })
}

#[derive(Clone, Copy)]
pub struct OtelBlobObserver;

impl BlobStoreObserver for OtelBlobObserver {
    fn put_bytes(&self, n: u64) {
        if n > 0 {
            let inst = ensure_instruments();
            inst.put_bytes.add(n, &[KeyValue::new("op", "put")]);
            let _ = PUT_ACC.fetch_add(n, Ordering::Relaxed);
        }
    }
    fn get_bytes(&self, n: u64) {
        if n > 0 {
            let inst = ensure_instruments();
            inst.get_bytes.add(n, &[KeyValue::new("op", "get")]);
            let _ = GET_ACC.fetch_add(n, Ordering::Relaxed);
        }
    }
    fn cleanup_count(&self, n: u64) {
        if n > 0 {
            let inst = ensure_instruments();
            inst.cleanup_count.add(n, &[KeyValue::new("op", "cleanup")]);
            let _ = CLEAN_ACC.fetch_add(n, Ordering::Relaxed);
        }
    }
    fn span(&self, name: &'static str) -> BlobSpan {
        let span = tracing::span!(tracing::Level::INFO, "blob", op = name);
        // Enter the span; guard exits on drop.
        let entered = span.entered();
        ::blob_store::BlobSpan::from_guard(entered)
    }
}

/// Returns a global &'static instance suitable for blob_store::set_observer().
pub fn global() -> &'static OtelBlobObserver {
    INSTANCE.get_or_init(|| {
        // Ensure instruments are ready up-front
        let _ = ensure_instruments();
        OtelBlobObserver
    })
}

/// Snapshot test mirrors (for integration tests)
pub fn snapshot_counters() -> (u64, u64, u64) {
    (
        PUT_ACC.load(Ordering::Relaxed),
        GET_ACC.load(Ordering::Relaxed),
        CLEAN_ACC.load(Ordering::Relaxed),
    )
}
