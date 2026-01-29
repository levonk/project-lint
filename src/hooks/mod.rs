use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Session
    SessionStart,
    SessionEnd,

    // Tool Use
    PreToolUse,
    PostToolUse,

    // File Operations
    PreReadCode,
    PostReadCode,
    PreWriteCode,
    PostWriteCode,

    // Command Execution
    PreRunCommand,
    PostRunCommand,

    // Interaction
    PreUserPrompt, // Called UserPromptSubmit in Claude, pre_user_prompt in Windsurf
    PostModelResponse, // post_cascade_response in Windsurf

    // Notifications/Permissions
    Notification,
    PermissionRequest,

    // Control
    Stop,
    SubagentStop,

    // Generic/Unknown
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLintEvent {
    pub event_type: EventType,
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub cwd: Option<PathBuf>,
    pub context: EventContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventContext {
    // File Context
    pub file_path: Option<PathBuf>,
    pub file_content: Option<String>,
    pub edits: Option<Vec<FileEdit>>,

    // Tool Context
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_result: Option<serde_json::Value>,

    // Command Context
    pub command: Option<String>,
    pub exit_code: Option<i32>,

    // Interaction Context
    pub user_prompt: Option<String>,
    pub model_response: Option<String>,

    // Metadata
    pub ide_source: String, // "windsurf", "claude", "kiro", "generic"
    pub original_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub old_string: Option<String>,
    pub new_string: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub decision: Decision,
    pub message: Option<String>,
    pub modified_input: Option<serde_json::Value>, // For modifying tool inputs or prompts
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny,
    Ask, // Request user confirmation
    Warn, // Allow but show warning
}

impl Default for HookResult {
    fn default() -> Self {
        Self {
            decision: Decision::Allow,
            message: None,
            modified_input: None,
        }
    }
}

pub trait EventMapper {
    fn map_event(&self, input: &str) -> crate::utils::Result<ProjectLintEvent>;
    fn format_response(&self, result: HookResult) -> crate::utils::Result<String>;
}

pub mod mappers;
pub mod engine;
pub mod logger;

pub use engine::RuleEngine;
pub use logger::{HookLogger, HookStats, initialize_global_logger, log_hook_event, get_hook_stats};
