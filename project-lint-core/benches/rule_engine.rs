//! Criterion benchmarks for the hook rule engine.
//!
//! Measures the hot path of `RuleEngine::evaluate_event` under a realistic
//! custom-rule set so regressions in event routing are caught.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use project_lint_core::config::{Config, CustomRule, ExecutionMode, RuleSeverity};
use project_lint_core::hooks::{EventContext, EventType, ProjectLintEvent, RuleEngine};
use serde_json::json;
use std::path::PathBuf;

fn make_config(rule_count: usize) -> Config {
    let mut config = Config::default();
    for i in 0..rule_count {
        config.rules.custom_rules.push(CustomRule {
            name: format!("bench-rule-{}", i),
            pattern: "*".to_string(),
            message: format!("bench message {}", i),
            severity: RuleSeverity::Warning,
            check_content: false,
            content_pattern: None,
            exception_pattern: None,
            condition: None,
            required: false,
            required_if_path_exists: None,
            disabled_if_path_exists: None,
            enabled_if_path_exists: None,
            exclude_patterns: vec![],
            protected_paths: vec![],
            protected_branches: vec![],
            triggers: vec!["pre_tool_use".to_string()],
            mode: ExecutionMode::LocalSync,
        });
    }
    config
}

fn make_event() -> ProjectLintEvent {
    ProjectLintEvent {
        event_type: EventType::PreToolUse,
        session_id: Some("bench".to_string()),
        timestamp: Some("2026-08-05T00:00:00Z".to_string()),
        cwd: Some(PathBuf::from("/tmp/bench")),
        context: EventContext {
            tool_name: Some("bash".to_string()),
            tool_input: Some(json!({ "input": "npm install" })),
            ide_source: "windsurf".to_string(),
            ..Default::default()
        },
    }
}

fn bench_evaluate_event(c: &mut Criterion) {
    let event = make_event();
    for &rule_count in &[1, 10, 50] {
        let config = make_config(rule_count);
        let engine = RuleEngine::new(&config);
        c.bench_function(&format!("evaluate_event/{}-rules", rule_count), |b| {
            b.iter(|| black_box(engine.evaluate_event(black_box(&event))))
        });
    }
}

criterion_group!(benches, bench_evaluate_event);
criterion_main!(benches);
