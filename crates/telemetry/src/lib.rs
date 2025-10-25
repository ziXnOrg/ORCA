//! Telemetry integration stubs (Phase 0 baseline OTel wiring to follow).

#![deny(unsafe_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("otel setup failed: {0}")]
    Otel(String),
}

/// Initialize structured logging (JSON) with env filter.
/// Set RUST_LOG, e.g., "info,telemetry=debug".
pub fn init_json_logging() {
    let fmt_layer = fmt::layer().json().with_current_span(true).with_span_list(true);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = Registry::default().with(filter).with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).ok();
}

/// Initialize OpenTelemetry tracer (optional; behind `otel` feature). No tracing subscriber hookup.
#[cfg(feature = "otel")]
pub fn init_otel(service_name: &str) -> Result<(), TelemetryError> {
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::trace as sdktrace;
    use opentelemetry_sdk::{runtime, Resource};

    let resource = Resource::new(vec![KeyValue::new("service.name", service_name.to_owned())]);
    let _tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().http())
        .with_trace_config(sdktrace::config().with_resource(resource))
        .install_batch(runtime::Tokio)
        .map_err(|e| TelemetryError::Otel(e.to_string()))?;
    Ok(())
}

#[cfg(feature = "otel")]
pub mod metrics {
    //! OTel metrics (OTLP) for budget usage.
    use super::TelemetryError;
    use once_cell::sync::OnceCell;
    use opentelemetry::global;
    use opentelemetry::metrics::{Counter, Histogram, Meter, Unit};

    static METRICS_INIT: OnceCell<()> = OnceCell::new();

    fn detect_service_name() -> String {
        std::env::var("OTEL_SERVICE_NAME")
            .or_else(|_| std::env::var("ORCA_SERVICE_NAME"))
            .unwrap_or_else(|_| "orchestrator".to_string())
    }

    fn init_metrics_from_env() -> Result<(), TelemetryError> {
        // Configure OTLP metrics pipeline via env (OTEL_EXPORTER_*).
        let _svc = detect_service_name();
        let provider = opentelemetry_otlp::new_pipeline()
            .metrics(opentelemetry_sdk::runtime::Tokio)
            .with_exporter(opentelemetry_otlp::new_exporter().http())
            .build()
            .map_err(|e| TelemetryError::Otel(e.to_string()))?;
        global::set_meter_provider(provider);
        Ok(())
    }

    /// Initialize (idempotent) global metrics provider from env.
    fn ensure_metrics_provider() {
        let _ = METRICS_INIT.get_or_init(|| {
            let _ = init_metrics_from_env();
        });
    }

    #[derive(Clone)]
    pub struct CounterWrap {
        counter: Counter<u64>,
        hist: Histogram<u64>,
    }

    impl CounterWrap {
        /// Add a value to the counter and record into histogram. Attributes ignored for now.
        pub fn add(&self, val: u64, _attrs: &[()]) {
            self.counter.add(val, &[]);
            self.hist.record(val, &[]);
        }
    }

    #[derive(Clone)]
    pub struct BudgetInstruments {
        tokens: CounterWrap,
        cost_micros: CounterWrap,
    }

    impl BudgetInstruments {
        pub fn tokens(&self) -> CounterWrap {
            self.tokens.clone()
        }
        pub fn cost_micros(&self) -> CounterWrap {
            self.cost_micros.clone()
        }
    }

    pub fn init_budget_instruments() -> BudgetInstruments {
        ensure_metrics_provider();
        let meter: Meter = global::meter("orca.budget");
        let tokens = CounterWrap {
            counter: meter
                .u64_counter("orca.tokens.total")
                .with_description("Total tokens recorded (monotonic)")
                .init(),
            hist: meter
                .u64_histogram("orca.tokens.per_task")
                .with_description("Tokens per task")
                .with_unit(Unit::new("1"))
                .init(),
        };
        let cost = CounterWrap {
            counter: meter
                .u64_counter("orca.cost.total_micros")
                .with_description("Total cost (micros) recorded (monotonic)")
                .with_unit(Unit::new("us"))
                .init(),
            hist: meter
                .u64_histogram("orca.cost.per_task_micros")
                .with_description("Cost per task (micros)")
                .with_unit(Unit::new("us"))
                .init(),
        };
        BudgetInstruments { tokens, cost_micros: cost }
    }
}

/// Returns whether telemetry is initialized (stubbed).
pub fn is_initialized() -> bool {
    true
}

#[derive(Clone, Default)]
pub struct BudgetMetrics {
    tokens_total: Arc<AtomicU64>,
    cost_total_micros: Arc<AtomicU64>,
}

impl BudgetMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&self, tokens: u64, cost_micros: u64) {
        if tokens > 0 {
            let _ = self.tokens_total.fetch_add(tokens, Ordering::Relaxed);
        }
        if cost_micros > 0 {
            let _ = self.cost_total_micros.fetch_add(cost_micros, Ordering::Relaxed);
        }
    }
    pub fn snapshot(&self) -> (u64, u64) {
        (self.tokens_total.load(Ordering::Relaxed), self.cost_total_micros.load(Ordering::Relaxed))
    }
}
