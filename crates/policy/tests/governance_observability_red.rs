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
    let mut eng = Engine::new();
    let yaml = r#"
rules:
  - name: Deny-Tools
    when: ToolInvocation
    action: deny
  - name: Flag-Prompts
    when: LLMPrompt
    action: allow_but_flag
"#;
    let tmp = std::env::temp_dir().join(format!("policy_obs_{}.yaml", std::process::id()));
    std::fs::write(&tmp, yaml).unwrap();
    eng.load_from_yaml_path(&tmp).unwrap();

    let env_tool = json!({"payload_json": "{\"tool\": \"curl\"}"});
    let env_prompt = json!({"payload_json": "hello"});
    let d1 = eng.pre_submit_task(&env_tool);
    let d2 = eng.pre_submit_task(&env_prompt);

    // Ensure we actually produced distinct decisions
    assert!(matches!(d1.kind, DecisionKind::Deny));
    assert!(matches!(d2.kind, DecisionKind::Allow));
}

