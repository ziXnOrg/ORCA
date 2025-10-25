use policy::Engine;
use std::fs;
use std::path::PathBuf;

fn write_temp_yaml(name: &str, content: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("policy_val_{}_{}_{}.yaml", name, std::process::id(), rand_suffix()));
    fs::write(&p, content).expect("write temp yaml");
    p
}

fn rand_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

#[test]
fn empty_file_errors() {
    let p = write_temp_yaml("empty", "");
    let mut eng = Engine::new();
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err());
}

#[test]
fn invalid_action_value_errors() {
    let yaml = r#"
rules:
  - name: Bad
    when: ToolInvocation
    action: approve
"#;
    let p = write_temp_yaml("bad_action", yaml);
    let mut eng = Engine::new();
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err(), "expected invalid action to error");
}

#[test]
fn duplicate_allowlist_errors() {
    let yaml = r#"
tool_allowlist:
  - echo
  - ECHO
rules: []
"#;
    let p = write_temp_yaml("dup_allow", yaml);
    let mut eng = Engine::new();
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err(), "expected duplicate allowlist to error");
}

#[test]
fn empty_string_in_allowlist_errors() {
    let yaml = r#"
tool_allowlist:
  - "  "
rules: []
"#;
    let p = write_temp_yaml("empty_allow", yaml);
    let mut eng = Engine::new();
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err(), "expected empty allowlist entry to error");
}

#[test]
fn malformed_regex_transform_errors() {
    let yaml = r#"
rules:
  - name: ModifyWithBadRegex
    when: pii_detect
    action: modify
    transform: "regex:(?"
"#;
    let p = write_temp_yaml("bad_regex", yaml);
    let mut eng = Engine::new();
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err(), "expected invalid regex to error");
}

#[test]
fn missing_required_fields_errors() {
    // Missing 'action' and 'when' should trigger a serde error -> mapped to our Err
    let yaml = r#"
rules:
  - name: NoAction
"#;
    let p = write_temp_yaml("missing_fields", yaml);
    let mut eng = Engine::new();
    let res = eng.load_from_yaml_path(&p);
    assert!(res.is_err(), "expected missing fields to error");
}
