use policy::{DecisionKind, Engine};
use serde_json::json;

// RED: This test intentionally references a not-yet-implemented OTel-backed observer for
// policy decisions to establish a compile-time RED state. The module `telemetry::policy_observer`
// and its `global()` function will be added in GREEN.
#[test]
fn otel_metrics_export_with_attributes() {
    // Install OTel-backed observer (does not exist yet → compile error for RED phase)
    let _ = policy::set_observer(Some(Box::new(telemetry::policy_observer::global())));

    // Drive a couple of decisions so that metrics would be emitted once GREEN is implemented
    // Use separate engines to avoid precedence interactions between deny and allow_but_flag.
    let mut eng_deny = Engine::new();
    let yaml_deny = r#"
rules:
  - name: Deny-Tools
    when: ToolInvocation
    action: deny
"#;
    let tmp1 = std::env::temp_dir().join(format!("policy_obs_deny_{}.yaml", std::process::id()));
    std::fs::write(&tmp1, yaml_deny).unwrap();
    eng_deny.load_from_yaml_path(&tmp1).unwrap();

    let mut eng_flag = Engine::new();
    let yaml_flag = r#"
rules:
  - name: Flag-Prompts
    when: LLMPrompt
    action: allow_but_flag
"#;
    let tmp2 = std::env::temp_dir().join(format!("policy_obs_flag_{}.yaml", std::process::id()));
    std::fs::write(&tmp2, yaml_flag).unwrap();
    eng_flag.load_from_yaml_path(&tmp2).unwrap();

    let env_tool = json!({"payload_json": "{\"tool\": \"curl\"}"});
    let env_prompt = json!({"payload_json": "hello"});
    let d1 = eng_deny.pre_submit_task(&env_tool);
    let d2 = eng_flag.pre_submit_task(&env_prompt);

    // Ensure we actually produced distinct decisions
    assert!(matches!(d1.kind, DecisionKind::Deny));
    assert!(matches!(d2.kind, DecisionKind::Allow));
}
