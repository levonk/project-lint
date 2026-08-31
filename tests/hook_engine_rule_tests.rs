use project_lint_core::config::{Config, CustomRule, ExecutionMode, RuleSeverity};
use project_lint_core::hooks::{Decision, EventContext, EventType, ProjectLintEvent, RuleEngine};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn ban_ts_rule(disabled_if_path_exists: Option<&str>, exclude_patterns: &[&str]) -> CustomRule {
    CustomRule {
        name: "ban_ts".to_string(),
        pattern: "**/*.ts".to_string(),
        message: "Ambiguous .ts".to_string(),
        severity: RuleSeverity::Warning,
        check_content: false,
        content_pattern: None,
        exception_pattern: None,
        condition: None,
        required: false,
        required_if_path_exists: None,
        disabled_if_path_exists: disabled_if_path_exists.map(|s| s.to_string()),
        enabled_if_path_exists: None,
        exclude_patterns: exclude_patterns.iter().map(|s| s.to_string()).collect(),
        protected_paths: vec![],
        protected_branches: vec![],
        triggers: vec!["pre_write_code".to_string()],
        mode: ExecutionMode::LocalSync,
    }
}

fn write_event(cwd: &std::path::Path, file_path: &str) -> ProjectLintEvent {
    ProjectLintEvent {
        event_type: EventType::PreWriteCode,
        session_id: None,
        timestamp: None,
        cwd: Some(cwd.to_path_buf()),
        context: EventContext {
            file_path: Some(PathBuf::from(file_path)),
            file_content: None,
            edits: None,
            tool_name: None,
            tool_input: None,
            tool_result: None,
            command: None,
            exit_code: None,
            cwd: None,
            user_prompt: None,
            model_response: None,
            ide_source: "claude".to_string(),
            original_payload: None,
        },
    }
}

#[test]
fn test_disabled_if_path_exists_skips_rule() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("next.config.mjs"), "export {};\n").unwrap();
    fs::create_dir_all(dir.path().join("app")).unwrap();
    fs::write(dir.path().join("app/page.ts"), "export {};\n").unwrap();

    let mut config = Config::default();
    config
        .rules
        .custom_rules
        .push(ban_ts_rule(Some("next.config.*"), &[]));

    let event = write_event(dir.path(), "app/page.ts");
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event).unwrap();

    assert_eq!(result.decision, Decision::Allow);
    assert!(result.message.is_none());
}

#[test]
fn test_disabled_if_path_exists_no_marker_flags() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::write(dir.path().join("lib/utils.ts"), "export {};\n").unwrap();

    let mut config = Config::default();
    config
        .rules
        .custom_rules
        .push(ban_ts_rule(Some("next.config.*"), &["**/*.d.ts"]));

    let event = write_event(dir.path(), "lib/utils.ts");
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event).unwrap();

    assert_eq!(result.decision, Decision::Warn);
    assert!(result.message.unwrap().contains("ban_ts"));
}

#[test]
fn test_exclude_patterns_exempt_d_ts_in_hook_engine() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("types.d.ts"), "export {};\n").unwrap();

    let mut config = Config::default();
    config
        .rules
        .custom_rules
        .push(ban_ts_rule(None, &["**/*.d.ts"]));

    let event = write_event(dir.path(), "types.d.ts");
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event).unwrap();

    assert_eq!(result.decision, Decision::Allow);
    assert!(result.message.is_none());
}

#[test]
fn test_no_triggers_means_rule_not_evaluated_by_hook() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::write(dir.path().join("lib/utils.ts"), "export {};\n").unwrap();

    let mut rule = ban_ts_rule(None, &[]);
    rule.triggers = vec![]; // no triggers -> never evaluated by hook engine
    let mut config = Config::default();
    config.rules.custom_rules.push(rule);

    let event = write_event(dir.path(), "lib/utils.ts");
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event).unwrap();

    assert_eq!(result.decision, Decision::Allow);
    assert!(result.message.is_none());
}

#[test]
fn test_enabled_if_path_exists_skips_non_ts_project_in_hook() {
    let dir = TempDir::new().unwrap();
    // No tsconfig.json -> NOT a TypeScript project -> rule skipped.
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/utils.ts"), "export {};\n").unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[package]\n").unwrap();

    let mut rule = ban_ts_rule(None, &[]);
    rule.enabled_if_path_exists = Some("tsconfig.json".to_string());
    let mut config = Config::default();
    config.rules.custom_rules.push(rule);

    let event = write_event(dir.path(), "src/utils.ts");
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event).unwrap();

    // Non-TS project -> rule not activated -> Allow.
    assert_eq!(result.decision, Decision::Allow);
    assert!(result.message.is_none());
}

#[test]
fn test_enabled_if_path_exists_activates_for_ts_project_in_hook() {
    let dir = TempDir::new().unwrap();
    // tsconfig.json present -> TypeScript project -> rule active.
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/utils.ts"), "export {};\n").unwrap();
    fs::write(dir.path().join("tsconfig.json"), "{}\n").unwrap();

    let mut rule = ban_ts_rule(None, &[]);
    rule.enabled_if_path_exists = Some("tsconfig.json".to_string());
    let mut config = Config::default();
    config.rules.custom_rules.push(rule);

    let event = write_event(dir.path(), "src/utils.ts");
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event).unwrap();

    // TS project -> rule active -> Warn.
    assert_eq!(result.decision, Decision::Warn);
    assert!(result.message.unwrap().contains("ban_ts"));
}
