use crate::hooks::{ProjectLintEvent, EventType, EventContext};
use crate::config::{Config, CustomRule, RuleSeverity};
use crate::hooks::engine::RuleEngine;
use std::path::PathBuf;
use serde_json::json;

#[test]
fn test_pnpm_enforcement_rule() {
    // Create a mock config with the pnpm rule
    let mut config = Config::default();
    config.rules.custom_rules.push(CustomRule {
        name: "pnpm-workspace-enforcer".to_string(),
        pattern: "*".to_string(),
        message: "Use pnpm instead".to_string(),
        severity: RuleSeverity::Warning,
        check_content: false,
        content_pattern: None,
        exception_pattern: None,
        condition: None,
        required: false,
        required_if_path_exists: None,
        triggers: vec!["pre_tool_use".to_string()],
    });

    // Create a mock event for npm command
    let event = ProjectLintEvent {
        event_type: EventType::PreToolUse,
        session_id: Some("test-session".to_string()),
        timestamp: Some("2025-01-28T20:00:00Z".to_string()),
        cwd: Some(PathBuf::from("/tmp/test-project")),
        context: EventContext {
            file_path: None,
            file_content: None,
            edits: None,
            tool_name: Some("bash".to_string()),
            tool_input: Some(json!({
                "input": "npm install express"
            })),
            tool_result: None,
            command: None,
            exit_code: None,
            user_prompt: None,
            model_response: None,
            ide_source: "windsurf".to_string(),
            original_payload: Some(json!({
                "agent_action_name": "pre_mcp_tool_use"
            })),
        },
    };

    // Evaluate the rule (this will check for pnpm workspace)
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event);

    // Should not trigger without pnpm workspace
    assert!(result.is_ok());
    let hook_result = result.unwrap();
    assert!(hook_result.message.is_none()); // No pnpm workspace detected
}

#[test]
fn test_command_extraction() {
    let engine = RuleEngine::new(&Config::default());
    
    // Test Windsurf format
    let windsurf_input = json!({
        "input": "npm run dev"
    });
    assert_eq!(
        engine.extract_command_from_input(&windsurf_input),
        Some("npm run dev".to_string())
    );
    
    // Test Claude format
    let claude_input = json!({
        "tool_input": "npm test"
    });
    assert_eq!(
        engine.extract_command_from_input(&claude_input),
        Some("npm test".to_string())
    );
    
    // Test generic format
    let generic_input = json!({
        "command": "npm build"
    });
    assert_eq!(
        engine.extract_command_from_input(&generic_input),
        Some("npm build".to_string())
    );
}
