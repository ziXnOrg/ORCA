use policy::Engine;
use serde_json::json;

#[test]
fn ssn_is_redacted_in_payload_json() {
    let eng = Engine::new();
    let mut env = json!({
        "payload_json": "User SSN: 123-45-6789"
    });
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, policy::DecisionKind::Modify | policy::DecisionKind::Allow));
    if let Some(modified) = d.payload {
        env = modified;
    }
    let s = env.get("payload_json").and_then(|v| v.as_str()).unwrap_or("").to_string();
    assert!(!s.contains("123-45-6789"));
    assert!(s.contains("[REDACTED]"));
}

#[test]
fn load_yaml_rules() {
    let mut eng = Engine::new();
    let p1 = std::path::Path::new("Docs/policy.yaml");
    let p2 = std::path::Path::new("../../Docs/policy.yaml");
    let path = if p1.exists() { p1 } else { p2 };
    eng.load_from_yaml_path(path).unwrap();
    // Not asserting specifics beyond successful load; future: check deny tool rule present
    let d = eng.pre_submit_task(&json!({"payload_json":"ok"}));
    assert!(matches!(d.kind, policy::DecisionKind::Allow | policy::DecisionKind::Modify | policy::DecisionKind::Deny));
}
