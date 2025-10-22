//! Policy engine interfaces (Phase 5 target; integrated in Phase 2).

#![deny(unsafe_code)]

use regex::Regex;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    Allow,
    Deny,
    Modify,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub kind: DecisionKind,
    pub payload: Option<Value>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Engine {
    pii: Regex,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    #[must_use]
    pub fn new() -> Self {
        let pii = Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap();
        Self { pii }
    }

    pub fn pre_start_run(&self, envelope: &Value) -> Decision {
        self.scan_and_redact(envelope)
    }

    pub fn pre_submit_task(&self, envelope: &Value) -> Decision {
        self.scan_and_redact(envelope)
    }

    pub fn post_submit_task(&self, _result: &Value) -> Decision {
        Decision { kind: DecisionKind::Allow, payload: None, reason: None }
    }

    fn scan_and_redact(&self, envelope: &Value) -> Decision {
        let mut modified = envelope.clone();
        let mut changed = false;
        if let Some(payload) =
            modified.get_mut("payload_json").and_then(|v| v.as_str()).map(|s| s.to_string())
        {
            let redacted = self.pii.replace_all(&payload, "[REDACTED]").into_owned();
            if redacted != payload {
                changed = true;
                // Replace string payload_json
                if let Some(v) = modified.get_mut("payload_json") {
                    *v = json!(redacted);
                }
            }
        }
        if changed {
            Decision {
                kind: DecisionKind::Modify,
                payload: Some(modified),
                reason: Some("PII redacted".into()),
            }
        } else {
            Decision { kind: DecisionKind::Allow, payload: None, reason: None }
        }
    }
}
