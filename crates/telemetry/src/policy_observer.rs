#![allow(clippy::module_name_repetitions)]

use once_cell::sync::OnceCell;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Meter};
use opentelemetry::KeyValue;

struct Instruments {
    counter: Counter<u64>,
}

static INSTR: OnceCell<Instruments> = OnceCell::new();

fn ensure_instruments() -> &'static Instruments {
    INSTR.get_or_init(|| {
        // Use the global meter provider (may be a no-op if OTLP not initialized).
        let meter: Meter = global::meter("orca.policy");
        let counter = meter
            .u64_counter("policy.decision.count")
            .with_description("Policy decision counter")
            .init();
        Instruments { counter }
    })
}

/// OTel-backed observer for policy decisions.
#[derive(Clone, Copy, Debug, Default)]
pub struct OtelPolicyObserver;

impl policy::PolicyObserver for OtelPolicyObserver {
    fn on_decision(&self, phase: &str, d: &policy::Decision) {
        let inst = ensure_instruments();
        let kind_str = match d.kind {
            policy::DecisionKind::Allow => "allow",
            policy::DecisionKind::Deny => "deny",
            policy::DecisionKind::Modify => "modify",
        };
        let action_str = d.action.as_deref().unwrap_or(kind_str);
        let attrs = [
            KeyValue::new("phase", phase.to_string()),
            KeyValue::new("kind", kind_str.to_string()),
            KeyValue::new("action", action_str.to_string()),
        ];
        inst.counter.add(1, &attrs);
        // Emit a secondary alias for allow_but_flag to plain "flag" for dashboards, if desired
        if action_str == "allow_but_flag" {
            let attrs2 = [
                KeyValue::new("phase", phase.to_string()),
                KeyValue::new("kind", kind_str.to_string()),
                KeyValue::new("action", "flag".to_string()),
            ];
            inst.counter.add(1, &attrs2);
        }
    }
}

/// Return an observer instance. Prefer a new value instead of &'static for simplicity.
pub fn global() -> OtelPolicyObserver {
    let _ = ensure_instruments();
    OtelPolicyObserver
}
