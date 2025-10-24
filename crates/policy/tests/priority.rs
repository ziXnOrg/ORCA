use policy::Engine;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn write_temp_yaml(name: &str, content: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("policy_test_{}_{}_{}.yaml", name, std::process::id(), rand_suffix()));
    fs::write(&p, content).expect("write temp yaml");
    p
}

fn rand_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

#[test]
fn deny_vs_allow_equal_priority_most_restrictive_wins() {
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
    let path = write_temp_yaml("prio1", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&path).unwrap();

    let env = json!({"payload_json":"ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, policy::DecisionKind::Deny));
    assert_eq!(d.rule_name.as_deref(), Some("Deny Tools"));
}

#[test]
fn allow_higher_priority_over_deny() {
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
    let path = write_temp_yaml("prio2", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&path).unwrap();

    let env = json!({"payload_json":"ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, policy::DecisionKind::Allow));
    assert_eq!(d.action.as_deref(), Some("allow_but_flag"));
    assert_eq!(d.rule_name.as_deref(), Some("Allow Flag High"));
}

#[test]
fn first_match_wins_on_equal_pri_equal_severity() {
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
    let path = write_temp_yaml("prio3", yaml);
    let mut eng = Engine::new();
    eng.load_from_yaml_path(&path).unwrap();

    let env = json!({"payload_json":"ok"});
    let d = eng.pre_submit_task(&env);
    assert!(matches!(d.kind, policy::DecisionKind::Deny));
    assert_eq!(d.rule_name.as_deref(), Some("Deny First"));
}

