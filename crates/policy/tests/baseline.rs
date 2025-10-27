use policy::{DecisionKind, Engine};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn write_temp_yaml(name: &str, content: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("policy_baseline_{}_{}_{}.yaml", name, std::process::id(), rand_suffix()));
    fs::write(&p, content).expect("write temp yaml");
    p
}

fn rand_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

#[test]
fn missing_policy_defaults_to_deny() {
    // No policy loaded -> fail-closed expected (RED): should Deny
    let eng = Engine::new();
    let env = json!({"payload_json": "ok"});
    let d = eng.pre_submit_task(&env);
    assert!(
        matches!(d.kind, DecisionKind::Deny),
        "expected Deny when no policy is loaded (fail-closed), got: {:?}",
        d
    );
}

#[test]
fn malformed_policy_defaults_to_deny() {
    // Malformed policy load should result in fail-closed Deny on evaluation (RED)
    let mut eng = Engine::new();
    let yaml = "rules: ["; // malformed YAML
    let p = write_temp_yaml("malformed", yaml);
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err(), "expected malformed YAML to error on load");

    let env = json!({"payload_json": "ok"});
    let d = eng.pre_submit_task(&env);
    assert!(
        matches!(d.kind, DecisionKind::Deny),
        "expected Deny on invalid policy state (fail-closed), got: {:?}",
        d
    );
}

#[test]
fn allow_but_flag_maps_to_allow_with_action() {
    let yaml = r#"
rules:
  - name: Flag Prompt
    when: LLMPrompt
    action: allow_but_flag
    priority: 10
    message: "flag for review"
"#;
    let p = write_temp_yaml("flag", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&p).unwrap();

    let env = json!({"payload_json": "ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Allow));
    assert_eq!(d.action.as_deref(), Some("allow_but_flag"));
    assert_eq!(d.rule_name.as_deref(), Some("Flag Prompt"));
    assert_eq!(d.reason.as_deref(), Some("flag for review"));
}

#[test]
fn modify_on_pii_redacts_payload() {
    let eng = Engine::new();
    let env = json!({"payload_json": "User SSN: 123-45-6789"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Modify));
    let modified = d.payload.expect("expected modified payload");
    let s = modified.get("payload_json").and_then(|v| v.as_str()).unwrap_or("").to_string();
    assert!(!s.contains("123-45-6789"));
    assert!(s.contains("[REDACTED]"));
    assert_eq!(d.rule_name.as_deref(), Some("builtin_redact_pii"));
    assert_eq!(d.action.as_deref(), Some("modify"));
    assert_eq!(d.reason.as_deref(), Some("PII redacted"));
}

#[test]
fn deny_action_maps_to_deny() {
    let yaml = r#"
rules:
  - name: Deny Tools
    when: ToolInvocation
    action: deny
    priority: 10
"#;
    let p = write_temp_yaml("deny", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&p).unwrap();

    // Include a tool in the payload to trigger tool checks / rule matching
    let env = json!({"payload_json": "{\"tool\":\"echo\"}"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Deny));
    assert_eq!(d.action.as_deref(), Some("deny"));
}

#[test]
fn higher_priority_overrides_lower() {
    let yaml = r#"
rules:
  - name: Deny Low
    when: ToolInvocation
    action: deny
    priority: 5
  - name: Allow Flag High
    when: LLMPrompt
    action: allow_but_flag
    priority: 50
"#;
    let p = write_temp_yaml("prio_hi", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&p).unwrap();

    let env = json!({"payload_json": "ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Allow));
    assert_eq!(d.action.as_deref(), Some("allow_but_flag"));
    assert_eq!(d.rule_name.as_deref(), Some("Allow Flag High"));
}

#[test]
fn most_restrictive_wins_on_equal_priority() {
    let yaml = r#"
rules:
  - name: Deny Tools
    when: ToolInvocation
    action: deny
    priority: 10
  - name: Flag Prompt
    when: LLMPrompt
    action: allow_but_flag
    priority: 10
"#;
    let p = write_temp_yaml("restrictive", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&p).unwrap();

    let env = json!({"payload_json": "ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Deny));
    assert_eq!(d.rule_name.as_deref(), Some("Deny Tools"));
}

#[test]
fn first_match_wins_full_tie() {
    let yaml = r#"
rules:
  - name: Deny First
    when: ToolInvocation
    action: deny
    priority: 7
  - name: Deny Second
    when: ToolInvocation
    action: deny
    priority: 7
"#;
    let p = write_temp_yaml("first_tie", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&p).unwrap();

    let env = json!({"payload_json": "ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, DecisionKind::Deny));
    assert_eq!(d.rule_name.as_deref(), Some("Deny First"));
}

#[test]
fn stable_decision_across_runs() {
    let yaml = r#"
rules:
  - name: Deny First
    when: ToolInvocation
    action: deny
    priority: 7
  - name: Deny Second
    when: ToolInvocation
    action: deny
    priority: 7
"#;
    let p = write_temp_yaml("stable", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&p).unwrap();

    let env = json!({"payload_json": "ok"});
    let d1 = eng.pre_submit_task(&env);
    let d2 = eng.pre_submit_task(&env);
    let d3 = eng.pre_submit_task(&env);
    assert_eq!(format!("{:?}", d1), format!("{:?}", d2));
    assert_eq!(format!("{:?}", d2), format!("{:?}", d3));
}

#[test]
fn tool_allowlist_enforced_and_default_deny_when_rule_present() {
    // A) allowlist enforced: tool not in allowlist -> Deny
    let yaml_a = r#"
tool_allowlist:
  - echo
rules: []
"#;
    let pa = write_temp_yaml("allowlist_a", yaml_a);
    let mut eng_a = Engine::new();
    eng_a.load_from_yaml_path(&pa).unwrap();
    let env_a = json!({"payload_json": "{\"tool\":\"curl\"}"});
    let da = eng_a.pre_submit_task(&env_a);
    assert!(matches!(da.kind, DecisionKind::Deny));
    assert_eq!(da.rule_name.as_deref(), Some("tool_allowlist"));

    // B) default deny on tool presence when a deny ToolInvocation rule exists
    let yaml_b = r#"
rules:
  - name: Deny Tools
    when: ToolInvocation
    action: deny
"#;
    let pb = write_temp_yaml("allowlist_b", yaml_b);
    let mut eng_b = Engine::new();
    eng_b.load_from_yaml_path(&pb).unwrap();
    let env_b = json!({"payload_json": "{\"tool\":\"echo\"}"});
    let db = eng_b.pre_submit_task(&env_b);
    assert!(matches!(db.kind, DecisionKind::Deny));
}
