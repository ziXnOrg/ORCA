//! RED phase acceptance tests for Governance baseline (T-6a-E2-POL-05)
//! These tests intentionally reference not-yet-implemented observability/audit hooks
//! to force a RED state. They will be satisfied in GREEN by wiring metrics and audit
//! emission around policy decisions and ensuring deterministic ordering semantics.

use policy::{DecisionKind, Engine};
use serde_json::json;

// --- Deterministic evaluation order (priority > restrictiveness > first-match) ---
#[test]
fn precedence_priority_then_restrictive_then_first_match() {
    let yaml = r#"
rules:
  - name: Deny Tools A
    when: ToolInvocation
    action: deny
    priority: 10
  - name: Deny Tools B
    when: ToolInvocation
    action: deny
    priority: 10
  - name: Flag Prompt
    when: LLMPrompt
    action: allow_but_flag
    priority: 10
"#;
    let path = write_temp_yaml("precedence", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&path).unwrap();

    let env = json!({"payload_json":"ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Deny));
    assert_eq!(d.rule_name.as_deref(), Some("Deny Tools A")); // first-match among equals
}

// --- Deny-on-error posture (malformed/missing policy) ---
#[test]
fn deny_on_error_malformed_policy() {
    let mut eng = Engine::new();
    let malformed = write_temp_yaml("malformed", "rules: [");
    let _ = eng.load_from_yaml_path(&malformed); // expect error, keep engine in fail-closed state
    let env = json!({"payload_json":"ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Deny));
}

// --- Tool allowlist enforcement ---
#[test]
fn tool_allowlist_blocks_non_allowed_tools() {
    let yaml = r#"
tool_allowlist:
  - echo
rules: []
"#;
    let path = write_temp_yaml("allowlist", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&path).unwrap();

    let env = json!({"payload_json":"{\"tool\":\"curl\"}"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Deny));
    assert_eq!(d.rule_name.as_deref(), Some("tool_allowlist"));
}

// --- Observability: metrics ---
// Expect a counter policy.decision.count with labels {phase, kind, action}
// These APIs do not exist yet; tests will fail to compile until GREEN.
#[test]
fn emits_policy_decision_metrics() {
    struct Capture;
    impl policy::PolicyObserver for Capture {
        fn on_decision(&self, phase: &str, d: &policy::Decision) {
            // Expect to be called exactly once per evaluation with stable attributes
            assert!(matches!(
                d.kind,
                policy::DecisionKind::Deny
                    | policy::DecisionKind::Modify
                    | policy::DecisionKind::Allow
            ));
            assert!(matches!(phase, "pre_submit_task" | "pre_start_run" | "post_submit_task"));
        }
    }
    policy::set_observer(Some(Box::new(Capture)));

    let mut eng = Engine::new();
    let yaml = r#"
rules:
  - name: Deny Tools
    when: ToolInvocation
    action: deny
"#;
    let path = write_temp_yaml("obs_metrics", yaml);
    eng.load_from_yaml_path(&path).unwrap();

    let env = json!({"payload_json":"{\"tool\":\"echo\"}"});
    let _ = eng.pre_submit_task(&env);

    // Validate metrics via to-be-provided handle (counter snapshot)
    let m = policy::policy_metrics();
    let c = m.decision_counter("pre_submit_task", "deny", "deny");
    assert!(c > 0, "expected policy.decision.count to increment for deny decision");
}

// --- Audit events ---
// Expect that an audit record is generated for each decision with rule_name/action
#[test]
fn emits_audit_event_per_decision() {
    let mut eng = Engine::new();
    let yaml = r#"
rules:
  - name: Flag Prompt
    when: LLMPrompt
    action: allow_but_flag
"#;
    let path = write_temp_yaml("audit", yaml);
    eng.load_from_yaml_path(&path).unwrap();

    // To-be-implemented API: install audit sink and capture records
    let sink = policy::install_audit_sink();

    let env = json!({"payload_json":"ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Allow));

    let records = sink.drain();
    assert!(records
        .iter()
        .any(|r| r.rule_name == Some("Flag Prompt".into())
            && r.action == Some("allow_but_flag".into())));
}

// Helpers
use std::fs;
use std::path::PathBuf;
fn write_temp_yaml(name: &str, content: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("policy_gov_{}_{}_{}.yaml", name, std::process::id(), rand_suffix()));
    fs::write(&p, content).expect("write temp yaml");
    p
}
fn rand_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}
