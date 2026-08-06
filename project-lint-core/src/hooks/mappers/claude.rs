use crate::hooks::{Decision, EventMapper, EventType, HookResult, ProjectLintEvent};
use crate::utils::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ClaudeMapper;

impl EventMapper for ClaudeMapper {
    fn map_event(&self, input: &str) -> Result<ProjectLintEvent> {
        let payload: Value = serde_json::from_str(input)?;
        let event_name = payload["hook_event_name"].as_str().unwrap_or_default();

        let event_type = match event_name {
            "PreToolUse" => EventType::PreToolUse,
            "PostToolUse" => EventType::PostToolUse,
            "UserPromptSubmit" => EventType::PreUserPrompt,
            "SessionStart" => EventType::SessionStart,
            "SessionEnd" => EventType::SessionEnd,
            "Stop" => EventType::Stop,
            "SubagentStop" => EventType::SubagentStop,
            "Notification" => EventType::Notification,
            "PermissionRequest" => EventType::PermissionRequest,
            _ => EventType::Unknown(event_name.to_string()),
        };

        let mut context = crate::hooks::EventContext {
            ide_source: "claude".to_string(),
            original_payload: Some(payload.clone()),
            ..Default::default()
        };

        // Common fields
        if let Some(cwd) = payload["cwd"].as_str() {
            context.cwd = Some(PathBuf::from(cwd));
        }

        match event_type {
            EventType::PreToolUse | EventType::PostToolUse => {
                context.tool_name = payload["tool_name"].as_str().map(|s| s.to_string());
                context.tool_input = Some(payload["tool_input"].clone());
                if event_type == EventType::PostToolUse {
                    context.tool_result = Some(payload["tool_response"].clone());
                }

                // Map specific tools to file context if applicable
                if let Some(name) = &context.tool_name {
                    match name.as_str() {
                        "Read" | "Edit" | "Write" => {
                            if let Some(input) = &context.tool_input {
                                if let Some(path) = input["file_path"].as_str() {
                                    context.file_path = Some(PathBuf::from(path));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            EventType::PreUserPrompt => {
                context.user_prompt = payload["prompt"].as_str().map(|s| s.to_string());
            }
            _ => {}
        }

        Ok(ProjectLintEvent {
            event_type,
            session_id: payload["session_id"].as_str().map(|s| s.to_string()),
            timestamp: None, // Claude payload doesn't seem to have a top-level timestamp in the examples
            cwd: context.cwd.clone(),
            context,
        })
    }

    fn format_response(&self, result: HookResult) -> Result<String> {
        let mut response = json!({
            "continue": true
        });

        match result.decision {
            Decision::Deny => {
                response["continue"] = json!(false);
                if let Some(msg) = result.message {
                    response["stopReason"] = json!(msg);
                }
            }
            Decision::Warn => {
                if let Some(msg) = result.message {
                    response["systemMessage"] = json!(msg);
                }
            }
            Decision::Allow => {
                if let Some(input) = result.modified_input {
                    // Claude supports modifying input for PreToolUse
                    response["hookSpecificOutput"] = json!({
                        "permissionDecision": "allow",
                        "updatedInput": input
                    });
                }
            }
            Decision::Ask => {
                // Claude specific 'ask' behavior isn't fully standard in simple JSON output usually
                // but we can map it to allow with a system message or deny.
                // Re-reading docs: PreToolUse supports "permissionDecision": "ask"
                response["hookSpecificOutput"] = json!({
                    "permissionDecision": "ask",
                    "permissionDecisionReason": result.message.unwrap_or_default()
                });
            }
        }

        Ok(serde_json::to_string(&response)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::Decision;
    use serde_json::json;

    #[test]
    fn test_map_pre_tool_use_read_extracts_file_path() -> Result<()> {
        let input = json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s-1",
            "cwd": "/repo",
            "tool_name": "Read",
            "tool_input": { "file_path": "/repo/src/lib.rs" }
        })
        .to_string();
        let event = ClaudeMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::PreToolUse);
        assert_eq!(event.session_id.as_deref(), Some("s-1"));
        assert_eq!(event.context.ide_source, "claude");
        assert_eq!(
            event.context.file_path.as_deref(),
            Some(std::path::Path::new("/repo/src/lib.rs"))
        );
        assert_eq!(event.context.tool_name.as_deref(), Some("Read"));
        Ok(())
    }

    #[test]
    fn test_map_user_prompt_submit() -> Result<()> {
        let input = json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "fix the bug"
        })
        .to_string();
        let event = ClaudeMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::PreUserPrompt);
        assert_eq!(event.context.user_prompt.as_deref(), Some("fix the bug"));
        Ok(())
    }

    #[test]
    fn test_map_unknown_event_name() -> Result<()> {
        let input = json!({ "hook_event_name": "SomethingNew" }).to_string();
        let event = ClaudeMapper.map_event(&input)?;
        assert_eq!(
            event.event_type,
            EventType::Unknown("SomethingNew".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_format_response_deny_sets_continue_false() -> Result<()> {
        let result = HookResult {
            decision: Decision::Deny,
            message: Some("blocked".to_string()),
            modified_input: None,
        };
        let out = ClaudeMapper.format_response(result)?;
        let v: Value = serde_json::from_str(&out)?;
        assert_eq!(v["continue"], json!(false));
        assert_eq!(v["stopReason"], json!("blocked"));
        Ok(())
    }

    #[test]
    fn test_format_response_allow_with_modified_input() -> Result<()> {
        let result = HookResult {
            decision: Decision::Allow,
            message: None,
            modified_input: Some(json!({ "cmd": "echo hi" })),
        };
        let out = ClaudeMapper.format_response(result)?;
        let v: Value = serde_json::from_str(&out)?;
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"],
            json!("allow")
        );
        assert_eq!(
            v["hookSpecificOutput"]["updatedInput"]["cmd"],
            json!("echo hi")
        );
        Ok(())
    }

    #[test]
    fn test_map_invalid_json_returns_error() {
        let result = ClaudeMapper.map_event("{ not valid json");
        assert!(result.is_err());
    }
}
