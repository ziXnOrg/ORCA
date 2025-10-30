//! ORCA core primitives and shared types.

#![deny(unsafe_code)]

/// Version of the ORCA core library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod ids {
    //! ID utilities: monotonic event ids and trace ids.

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    /// Generate a new monotonic identifier (starts at 1).
    pub fn next_monotonic_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Milliseconds since UNIX epoch (for timestamps).
    pub fn now_ms() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
    }

    /// Opaque trace identifier (UUID v4 string).
    pub fn new_trace_id() -> String {
        Uuid::new_v4().to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn monotonic_increments() {
            let a = next_monotonic_id();
            let b = next_monotonic_id();
            assert!(b > a);
        }

        #[test]
        fn trace_id_format() {
            let t = new_trace_id();
            assert_eq!(t.len(), 36);
            assert!(t.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        }
    }
}

pub mod envelope {
    //! Message envelope schema for tasks/results/errors.

    use super::ids::{new_trace_id, next_monotonic_id, now_ms};
    use serde::{Deserialize, Serialize};
    use serde_json::Value as JsonValue;

    /// Message type classification.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum MessageType {
        AgentTask,
        AgentResult,
        AgentError,
    }

    /// Standardized message envelope for cross-component communication.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Envelope {
        /// Unique message id.
        pub id: String,
        /// Parent id for causality (result/error links to task).
        pub parent_id: Option<String>,
        /// Trace id for correlating all messages in a workflow.
        pub trace_id: String,
        /// Agent role or name.
        pub agent: String,
        /// Message type.
        #[serde(rename = "type")]
        pub kind: MessageType,
        /// Payload body (schema varies by message type; JSON for flexibility in Phase 0).
        pub payload: JsonValue,
        /// Timeout budget in milliseconds (if applicable).
        pub timeout_ms: Option<u64>,
        /// Protocol version for forward-compat.
        pub protocol_version: u32,
        /// Creation timestamp.
        pub ts_ms: u64,
    }

    impl Envelope {
        /// Construct a new task envelope with a fresh id and trace.
        pub fn new_task(
            agent: impl Into<String>,
            payload: JsonValue,
            timeout_ms: Option<u64>,
        ) -> Self {
            let id = format!("msg-{}", next_monotonic_id());
            Self {
                id,
                parent_id: None,
                trace_id: new_trace_id(),
                agent: agent.into(),
                kind: MessageType::AgentTask,
                payload,
                timeout_ms,
                protocol_version: 1,
                ts_ms: now_ms(),
            }
        }

        /// Construct a result linked to a parent id within an existing trace.
        pub fn new_result(
            parent_id: impl Into<String>,
            trace_id: impl Into<String>,
            agent: impl Into<String>,
            payload: JsonValue,
        ) -> Self {
            let id = format!("msg-{}", next_monotonic_id());
            Self {
                id,
                parent_id: Some(parent_id.into()),
                trace_id: trace_id.into(),
                agent: agent.into(),
                kind: MessageType::AgentResult,
                payload,
                timeout_ms: None,
                protocol_version: 1,
                ts_ms: now_ms(),
            }
        }

        /// Construct an error linked to a parent id within an existing trace.
        pub fn new_error(
            parent_id: impl Into<String>,
            trace_id: impl Into<String>,
            agent: impl Into<String>,
            payload: JsonValue,
        ) -> Self {
            let id = format!("msg-{}", next_monotonic_id());
            Self {
                id,
                parent_id: Some(parent_id.into()),
                trace_id: trace_id.into(),
                agent: agent.into(),
                kind: MessageType::AgentError,
                payload,
                timeout_ms: None,
                protocol_version: 1,
                ts_ms: now_ms(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn task_envelope_defaults() {
            let e = Envelope::new_task("TestAgent", JsonValue::Null, Some(30_000));
            assert_eq!(e.kind, MessageType::AgentTask);
            assert!(e.parent_id.is_none());
            assert_eq!(e.protocol_version, 1);
            assert!(e.trace_id.len() >= 32);
        }

        #[test]
        fn result_links() {
            let task = Envelope::new_task("A", JsonValue::Null, None);
            let res =
                Envelope::new_result(task.id.clone(), task.trace_id.clone(), "A", JsonValue::Null);
            assert_eq!(res.parent_id.as_deref(), Some(task.id.as_str()));
            assert_eq!(res.trace_id, task.trace_id);
        }
    }
}

pub mod metadata {
    //! Unified metadata schema validation (v1).
    use jsonschema::{Draft, JSONSchema};
    use once_cell::sync::Lazy;
    use serde_json::Value;

    static SCHEMA_JSON: &str = include_str!("../../../Docs/metadata.schema.json");
    static COMPILED: Lazy<JSONSchema> = Lazy::new(|| {
        let schema: Value = serde_json::from_str(SCHEMA_JSON).expect("invalid schema json");
        JSONSchema::options().with_draft(Draft::Draft7).compile(&schema).expect("compile schema")
    });

    /// Validate a JSON value against the v1 metadata schema.
    pub fn validate_envelope(v: &Value) -> Result<(), String> {
        match COMPILED.validate(v) {
            Ok(_) => Ok(()),
            Err(iter) => {
                let msg = iter.map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
                Err(msg)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        #[test]
        fn valid_envelope() {
            let v = json!({
                "id": "m1", "trace_id": "t", "agent": "A", "kind": "agent_task", "protocol_version": 1, "ts_ms": 1
            });
            assert!(validate_envelope(&v).is_ok());
        }

        #[test]
        fn invalid_version() {
            let v = json!({
                "id": "m1", "trace_id": "t", "agent": "A", "kind": "agent_task", "protocol_version": 2, "ts_ms": 1
            });
            assert!(validate_envelope(&v).is_err());
        }
    }
}
