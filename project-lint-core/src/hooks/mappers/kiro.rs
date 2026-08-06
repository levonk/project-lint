use crate::hooks::{Decision, EventMapper, EventType, HookResult, ProjectLintEvent};
use crate::utils::Result;
use serde_json::Value;
use std::path::PathBuf;

pub struct KiroMapper;

impl EventMapper for KiroMapper {
    fn map_event(&self, input: &str) -> Result<ProjectLintEvent> {
        let payload: Value = serde_json::from_str(input)?;

        // Based on typical Kiro event structures found in docs
        let event_name = payload["event"]
            .as_str()
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
        if let Some(path) = payload["file"]
            .as_str()
            .or_else(|| payload["path"].as_str())
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::Decision;
    use serde_json::json;

    #[test]
    fn test_map_file_save_to_post_write_code() -> Result<()> {
        let input = json!({
            "event": "file_save",
            "session_id": "k-1",
            "file": "/repo/src/lib.rs"
        })
        .to_string();
        let event = KiroMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::PostWriteCode);
        assert_eq!(event.session_id.as_deref(), Some("k-1"));
        assert_eq!(event.context.ide_source, "kiro");
        assert_eq!(
            event.context.file_path.as_deref(),
            Some(std::path::Path::new("/repo/src/lib.rs"))
        );
        Ok(())
    }

    #[test]
    fn test_map_prompt_submit_to_pre_user_prompt() -> Result<()> {
        let input = json!({
            "type": "prompt.submit",
            "prompt": "refactor this"
        })
        .to_string();
        let event = KiroMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::PreUserPrompt);
        assert_eq!(event.context.user_prompt.as_deref(), Some("refactor this"));
        Ok(())
    }

    #[test]
    fn test_map_unknown_event() -> Result<()> {
        let input = json!({ "event": "mystery" }).to_string();
        let event = KiroMapper.map_event(&input)?;
        assert_eq!(event.event_type, EventType::Unknown("mystery".to_string()));
        Ok(())
    }

    #[test]
    fn test_map_invalid_json_returns_error() {
        let result = KiroMapper.map_event("{ not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_response_is_empty_string() -> Result<()> {
        let result = HookResult {
            decision: Decision::Deny,
            message: Some("blocked".to_string()),
            modified_input: None,
        };
        assert_eq!(KiroMapper.format_response(result)?, "");
        Ok(())
    }
}
