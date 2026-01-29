use crate::hooks::{Decision, EventMapper, EventType, HookResult, ProjectLintEvent};
use crate::utils::Result;
use serde_json::Value;
use std::path::PathBuf;

pub struct KiroMapper;

impl EventMapper for KiroMapper {
    fn map_event(&self, input: &str) -> Result<ProjectLintEvent> {
        let payload: Value = serde_json::from_str(input)?;

        // Based on typical Kiro event structures found in docs
        let event_name = payload["event"].as_str()
            .or_else(|| payload["type"].as_str())
            .unwrap_or_default();

        let event_type = match event_name {
            "file_save" | "file.save" => EventType::PostWriteCode,
            "file_create" | "file.create" => EventType::PostWriteCode,
            "prompt_submit" | "prompt.submit" => EventType::PreUserPrompt,
            "turn_complete" | "turn.complete" => EventType::PostModelResponse,
            _ => EventType::Unknown(event_name.to_string()),
        };

        let mut context = crate::hooks::EventContext {
            ide_source: "kiro".to_string(),
            original_payload: Some(payload.clone()),
            ..Default::default()
        };

        // Map fields
        if let Some(path) = payload["file"].as_str().or_else(|| payload["path"].as_str()) {
            context.file_path = Some(PathBuf::from(path));
        }

        if let Some(prompt) = payload["prompt"].as_str() {
            context.user_prompt = Some(prompt.to_string());
        }

        Ok(ProjectLintEvent {
            event_type,
            session_id: payload["session_id"].as_str().map(|s| s.to_string()),
            timestamp: None,
            cwd: None,
            context,
        })
    }

    fn format_response(&self, result: HookResult) -> Result<String> {
        // Default to no-op for now as Kiro response schema for shell hooks is usually just exit code
        Ok("".to_string())
    }
}
