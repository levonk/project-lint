use crate::hooks::{Decision, EventMapper, EventType, HookResult, ProjectLintEvent};
use crate::utils::Result;
use serde_json::Value;
use std::path::PathBuf;

pub struct WindsurfMapper;

impl EventMapper for WindsurfMapper {
    fn map_event(&self, input: &str) -> Result<ProjectLintEvent> {
        let payload: Value = serde_json::from_str(input)?;
        let action_name = payload["agent_action_name"].as_str().unwrap_or_default();
        let tool_info = &payload["tool_info"];

        let event_type = match action_name {
            "pre_read_code" => EventType::PreReadCode,
            "post_read_code" => EventType::PostReadCode,
            "pre_write_code" => EventType::PreWriteCode,
            "post_write_code" => EventType::PostWriteCode,
            "pre_run_command" => EventType::PreRunCommand,
            "post_run_command" => EventType::PostRunCommand,
            "pre_mcp_tool_use" => EventType::PreToolUse,
            "post_mcp_tool_use" => EventType::PostToolUse,
            "pre_user_prompt" => EventType::PreUserPrompt,
            "post_cascade_response" => EventType::PostModelResponse,
            _ => EventType::Unknown(action_name.to_string()),
        };

        let mut context = crate::hooks::EventContext {
            ide_source: "windsurf".to_string(),
            original_payload: Some(payload.clone()),
            ..Default::default()
        };

        // Map fields based on event type
        match event_type {
            EventType::PreReadCode | EventType::PostReadCode => {
                if let Some(path) = tool_info["file_path"].as_str() {
                    context.file_path = Some(PathBuf::from(path));
                }
            }
            EventType::PreWriteCode | EventType::PostWriteCode => {
                if let Some(path) = tool_info["file_path"].as_str() {
                    context.file_path = Some(PathBuf::from(path));
                }
                if let Some(edits) = tool_info["edits"].as_array() {
                    let mapped_edits = edits
                        .iter()
                        .map(|e| {
                            crate::hooks::FileEdit {
                                old_string: e["old_string"].as_str().map(|s| s.to_string()),
                                new_string: e["new_string"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                start_line: None, // Windsurf doesn't provide line numbers directly in edits array usually, strictly string replacement
                                end_line: None,
                            }
                        })
                        .collect();
                    context.edits = Some(mapped_edits);
                }
            }
            EventType::PreRunCommand | EventType::PostRunCommand => {
                context.command = tool_info["command_line"].as_str().map(|s| s.to_string());
                if let Some(cwd) = tool_info["cwd"].as_str() {
                    context.cwd = Some(PathBuf::from(cwd));
                }
            }
            EventType::PreToolUse | EventType::PostToolUse => {
                context.tool_name = tool_info["mcp_tool_name"].as_str().map(|s| s.to_string());
                context.tool_input = Some(tool_info["mcp_tool_arguments"].clone());
                if event_type == EventType::PostToolUse {
                    context.tool_result = Some(tool_info["mcp_result"].clone());
                }
            }
            EventType::PreUserPrompt => {
                context.user_prompt = tool_info["user_prompt"].as_str().map(|s| s.to_string());
            }
            EventType::PostModelResponse => {
                context.model_response = tool_info["response"].as_str().map(|s| s.to_string());
            }
            _ => {}
        }

        Ok(ProjectLintEvent {
            event_type,
            session_id: payload["trajectory_id"].as_str().map(|s| s.to_string()),
            timestamp: payload["timestamp"].as_str().map(|s| s.to_string()),
            cwd: context.cwd.clone(), // Windsurf provides cwd in tool_info for commands, but maybe not top level?
            context,
        })
    }

    fn format_response(&self, result: HookResult) -> Result<String> {
        // Windsurf can accept JSON responses for tool input modification
        let mut response = serde_json::Map::new();

        match result.decision {
            Decision::Deny => {
                // Block the action with exit code 2
                if let Some(msg) = result.message {
                    eprintln!("{}", msg); // Print reason to stderr
                }
                response.insert(
                    "decision".to_string(),
                    serde_json::Value::String("deny".to_string()),
                );
            }
            Decision::Warn => {
                // Allow but show warning
                if let Some(msg) = result.message {
                    eprintln!("⚠️  {}", msg);
                }
                response.insert(
                    "decision".to_string(),
                    serde_json::Value::String("warn".to_string()),
                );

                // If there's a modified input, include it for command rewriting
                if let Some(modified_input) = result.modified_input {
                    response.insert("modified_input".to_string(), modified_input);
                }
            }
            Decision::Ask => {
                // Request user confirmation
                if let Some(msg) = result.message {
                    println!("❓ {}", msg);
                }
                response.insert(
                    "decision".to_string(),
                    serde_json::Value::String("ask".to_string()),
                );
            }
            Decision::Allow => {
                response.insert(
                    "decision".to_string(),
                    serde_json::Value::String("allow".to_string()),
                );
            }
        }

        // Return JSON response if we have meaningful content
        if !response.is_empty() {
            Ok(serde_json::to_string(&response)?)
        } else {
            Ok("".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::Decision;
    use serde_json::json;

    #[test]
    fn test_map_pre_write_code_extracts_file_and_edits() -> Result<()> {
        let input = json!({
            "agent_action_name": "pre_write_code",
            "trajectory_id": "t-1",
            "timestamp": "2026-08-05T00:00:00Z",
            "tool_info": {
                "file_path": "/repo/src/main.rs",
                "edits": [
                    { "old_string": "a", "new_string": "b" }
                ]
            }
        })
        .to_string();
        let event = WindsurfMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::PreWriteCode);
        assert_eq!(event.session_id.as_deref(), Some("t-1"));
        assert_eq!(event.timestamp.as_deref(), Some("2026-08-05T00:00:00Z"));
        assert_eq!(
            event.context.file_path.as_deref(),
            Some(std::path::Path::new("/repo/src/main.rs"))
        );
        let edits = event.context.edits.expect("edits");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_string, "b");
        Ok(())
    }

    #[test]
    fn test_map_pre_run_command_extracts_command_and_cwd() -> Result<()> {
        let input = json!({
            "agent_action_name": "pre_run_command",
            "tool_info": { "command_line": "npm test", "cwd": "/repo" }
        })
        .to_string();
        let event = WindsurfMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::PreRunCommand);
        assert_eq!(event.context.command.as_deref(), Some("npm test"));
        assert_eq!(
            event.context.cwd.as_deref(),
            Some(std::path::Path::new("/repo"))
        );
        Ok(())
    }

    #[test]
    fn test_map_unknown_action() -> Result<()> {
        let input = json!({
            "agent_action_name": "mystery",
            "tool_info": {}
        })
        .to_string();
        let event = WindsurfMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::Unknown("mystery".to_string()));
        Ok(())
    }

    #[test]
    fn test_format_response_deny_emits_decision() -> Result<()> {
        let result = HookResult {
            decision: Decision::Deny,
            message: Some("nope".to_string()),
            modified_input: None,
        };
        let out = WindsurfMapper.format_response(result)?;
        let v: Value = serde_json::from_str(&out)?;
        assert_eq!(v["decision"], json!("deny"));
        Ok(())
    }

    #[test]
    fn test_format_response_allow_emits_decision() -> Result<()> {
        let result = HookResult {
            decision: Decision::Allow,
            message: None,
            modified_input: None,
        };
        let out = WindsurfMapper.format_response(result)?;
        let v: Value = serde_json::from_str(&out)?;
        assert_eq!(v["decision"], json!("allow"));
        Ok(())
    }

    #[test]
    fn test_map_invalid_json_returns_error() {
        let result = WindsurfMapper.map_event("{ broken");
        assert!(result.is_err());
    }
}
