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
                    let mapped_edits = edits.iter().map(|e| {
                        crate::hooks::FileEdit {
                            old_string: e["old_string"].as_str().map(|s| s.to_string()),
                            new_string: e["new_string"].as_str().unwrap_or_default().to_string(),
                            start_line: None, // Windsurf doesn't provide line numbers directly in edits array usually, strictly string replacement
                            end_line: None,
                        }
                    }).collect();
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
        // Windsurf primarily uses exit codes for blocking (exit 2).
        // It doesn't strictly define a JSON output schema for modification in the docs I read,
        // except implicitly it might log stdout.
        // For now, we'll return an empty string or log message. The runner will handle exit code.

        if result.decision == Decision::Deny {
            // The command runner should convert this to exit code 2
            if let Some(msg) = result.message {
                eprintln!("{}", msg); // Print reason to stderr
            }
            return Ok("".to_string());
        }

        Ok("".to_string())
    }
}
