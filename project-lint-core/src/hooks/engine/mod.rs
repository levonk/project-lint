use crate::config::{Config, CustomRule, ModularRule, RuleSeverity};
use crate::hooks::{Decision, HookResult, ProjectLintEvent};
use crate::utils::{matches_pattern, path_exists_glob, Result};
use serde_json;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

pub struct RuleEngine<'a> {
    config: &'a Config,
}

impl<'a> RuleEngine<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub fn evaluate_event(&self, event: &ProjectLintEvent) -> Result<HookResult> {
        let mut result = HookResult::default();
        let mut issues = Vec::new();

        // 1. Evaluate modular rules
        for rule in &self.config.modular_rules {
            if !rule.enabled {
                continue;
            }

            if self.matches_triggers(&rule.triggers, event)? {
                debug!("Rule '{}' triggered by event", rule.name);
                if let Some(custom_rules) = &rule.rules {
                    for custom_rule in custom_rules {
                        if let Some(issue) = self.evaluate_custom_rule(custom_rule, event)? {
                            issues.push(issue);
                        }
                    }
                }
            }
        }

        // 2. Evaluate top-level custom rules
        for rule in &self.config.rules.custom_rules {
            if self.matches_triggers(&rule.triggers, event)? {
                debug!("Top-level rule '{}' triggered by event", rule.name);
                if let Some(issue) = self.evaluate_custom_rule(rule, event)? {
                    issues.push(issue);
                }
            }
        }

        // 3. Process issues and determine result
        if !issues.is_empty() {
            let has_errors = issues
                .iter()
                .any(|i| matches!(i.severity, RuleSeverity::Error));

            let mut message = String::from("Project Lint violations detected:\n");
            let mut modified_input: Option<serde_json::Value> = None;

            for issue in &issues {
                let icon = match issue.severity {
                    RuleSeverity::Error => "❌",
                    RuleSeverity::Warning => "⚠️",
                    RuleSeverity::Info => "ℹ️",
                };
                message.push_str(&format!("{} {}: {}\n", icon, issue.name, issue.message));

                // Check if this issue includes a command rewrite suggestion
                if issue.name == "pnpm-workspace-enforcer" {
                    {
                        if let Some(tool_input) = &event.context.tool_input {
                            if let Some(command_str) = self.extract_command_from_input(tool_input) {
                                if command_str.starts_with("npm ") {
                                    let rewritten_command = command_str.replace("npm ", "pnpm ");
                                    // Create modified input with the rewritten command
                                    if tool_input.get("input").is_some() {
                                        let mut new_input = tool_input.clone();
                                        new_input["input"] =
                                            serde_json::Value::String(rewritten_command);
                                        modified_input = Some(new_input);
                                    } else if tool_input.get("tool_input").is_some() {
                                        let mut new_input = tool_input.clone();
                                        new_input["tool_input"] =
                                            serde_json::Value::String(rewritten_command);
                                        modified_input = Some(new_input);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            result.message = Some(message);
            result.modified_input = modified_input;

            if has_errors {
                result.decision = Decision::Deny;
            } else {
                result.decision = Decision::Warn;
            }
        }

        Ok(result)
    }

    fn matches_triggers(&self, triggers: &[String], event: &ProjectLintEvent) -> Result<bool> {
        if triggers.is_empty() {
            return Ok(false);
        }

        let event_type_str = serde_json::to_string(&event.event_type)?
            .trim_matches('"')
            .to_string();

        for trigger in triggers {
            if trigger == "all" || trigger == &event_type_str {
                return Ok(true);
            }

            // IDE specific triggers
            if let Some(original_payload) = &event.context.original_payload {
                if let Some(action_name) = original_payload["agent_action_name"].as_str() {
                    if trigger == action_name {
                        return Ok(true);
                    }
                }
                if let Some(hook_event_name) = original_payload["hook_event_name"].as_str() {
                    if trigger == hook_event_name {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn evaluate_custom_rule(
        &self,
        rule: &CustomRule,
        event: &ProjectLintEvent,
    ) -> Result<Option<DetectedIssue>> {
        // Resolve the project root once for both gates.
        let cwd_buf = event
            .cwd
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        // Project-level activation gate: only evaluate if the marker exists.
        if let Some(enable_spec) = &rule.enabled_if_path_exists {
            if !path_exists_glob(cwd_buf.as_path(), enable_spec) {
                debug!(
                    "Skipping rule '{}' because activation marker '{}' does not exist at project root",
                    rule.name, enable_spec
                );
                return Ok(None);
            }
        }

        // Project-level kill switch: skip the whole rule if a matching file exists.
        if let Some(disable_spec) = &rule.disabled_if_path_exists {
            if path_exists_glob(cwd_buf.as_path(), disable_spec) {
                debug!(
                    "Skipping rule '{}' because disable marker '{}' exists at project root",
                    rule.name, disable_spec
                );
                return Ok(None);
            }
        }

        // For event hooks, we mainly check context like file path or prompt content
        let mut matched = false;

        // Check if rule mentions a pattern that matches event file path.
        // Prefer glob::Pattern for full `**/*.ext` support (parity with the lint
        // command); fall back to the simple matches_pattern for legacy patterns.
        if let Some(file_path) = &event.context.file_path {
            let path_str = file_path.to_string_lossy();
            if crate::utils::is_glob(&rule.pattern) {
                if let Ok(glob_pat) = glob::Pattern::new(&rule.pattern) {
                    if glob_pat.matches(&path_str) {
                        matched = true;
                    }
                }
            }
            if !matched && crate::utils::matches_pattern(&path_str, &rule.pattern) {
                matched = true;
            }
        }

        // If no file path match, maybe the rule is generic but triggered by event
        if !matched && rule.pattern == "*" {
            matched = true;
        }

        if !matched {
            return Ok(None);
        }

        // Per-file exclusion: skip if the file path matches any exclude pattern.
        if !rule.exclude_patterns.is_empty() {
            if let Some(file_path) = &event.context.file_path {
                let path_str = file_path.to_string_lossy();
                let file_name = file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                for exclude in &rule.exclude_patterns {
                    let excluded = if crate::utils::is_glob(exclude) {
                        glob::Pattern::new(exclude)
                            .map(|p| p.matches(&path_str) || p.matches(&file_name))
                            .unwrap_or(false)
                    } else {
                        matches_pattern(&path_str, exclude) || matches_pattern(&file_name, exclude)
                    };
                    if excluded {
                        debug!(
                            "Rule '{}' matched '{}' but excluded by exclude_patterns",
                            rule.name, path_str
                        );
                        return Ok(None);
                    }
                }
            }
        }

        // Special handling for pnpm enforcement
        if rule.name == "pnpm-workspace-enforcer" {
            return self.evaluate_pnpm_rule(rule, event);
        }

        // Check content patterns against user prompt or file content if available
        if rule.check_content {
            let mut content_to_check = String::new();
            if let Some(prompt) = &event.context.user_prompt {
                content_to_check.push_str(prompt);
            }
            if let Some(file_content) = &event.context.file_content {
                content_to_check.push_str(file_content);
            }

            if let Some(pattern) = &rule.content_pattern {
                let contains = content_to_check.contains(pattern);
                let is_violation = match rule.condition.as_deref() {
                    Some("must_contain") => !contains,
                    _ => contains, // default is denylist
                };

                if is_violation {
                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: rule.message.clone(),
                        severity: rule.severity.clone(),
                    }));
                }
            }
        } else if !rule.required {
            // If it's a denylist rule (not required) and we matched the pattern, it's an issue
            return Ok(Some(DetectedIssue {
                name: rule.name.clone(),
                message: rule.message.clone(),
                severity: rule.severity.clone(),
            }));
        }

        Ok(None)
    }
}

pub struct DetectedIssue {
    pub name: String,
    pub message: String,
    pub severity: RuleSeverity,
}

impl<'a> RuleEngine<'a> {
    /// Evaluate pnpm workspace enforcement rule
    fn evaluate_pnpm_rule(
        &self,
        rule: &CustomRule,
        event: &ProjectLintEvent,
    ) -> Result<Option<DetectedIssue>> {
        // Only check on PreToolUse events
        if event.event_type != crate::hooks::EventType::PreToolUse {
            return Ok(None);
        }

        // Get the current working directory
        let cwd_buf = event
            .cwd
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let cwd = cwd_buf.as_path();

        // Check if this is a pnpm workspace
        if !self.is_pnpm_workspace(cwd)? {
            debug!("Not a pnpm workspace, skipping pnpm enforcement");
            return Ok(None);
        }

        // Check tool input for npm commands
        if let Some(tool_input) = &event.context.tool_input {
            if let Some(command_str) = self.extract_command_from_input(tool_input) {
                if command_str.starts_with("npm ") {
                    info!("Detected npm command in pnpm workspace: {}", command_str);

                    let rewritten_command = command_str.replace("npm ", "pnpm ");

                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: format!(
                            "🚫 This project uses pnpm (detected in package.json).\n\nFound: {}\nSuggested: {}\n\nThe command has been automatically rewritten to use pnpm.",
                            command_str, rewritten_command
                        ),
                        severity: rule.severity.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Check if the current directory is a pnpm workspace
    fn is_pnpm_workspace(&self, project_path: &Path) -> Result<bool> {
        let package_json_path = project_path.join("package.json");

        if !package_json_path.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(package_json_path)?;

        // Parse package.json
        if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&content) {
            // Check for pnpm packageManager field
            if let Some(package_manager) = package_json.get("packageManager") {
                if let Some(pm_str) = package_manager.as_str() {
                    if pm_str.starts_with("pnpm") {
                        info!("Detected pnpm workspace via packageManager: {}", pm_str);
                        return Ok(true);
                    }
                }
            }

            // Check for pnpm-workspace.yaml
            if project_path.join("pnpm-workspace.yaml").exists()
                || project_path.join("pnpm-workspace.yml").exists()
            {
                info!("Detected pnpm workspace via pnpm-workspace.yaml");
                return Ok(true);
            }

            // Check for pnpm-lock.yaml
            if project_path.join("pnpm-lock.yaml").exists() {
                info!("Detected pnpm workspace via pnpm-lock.yaml");
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Extract command string from tool input
    fn extract_command_from_input(&self, tool_input: &serde_json::Value) -> Option<String> {
        // Handle different IDE formats

        // Windsurf format
        if let Some(input) = tool_input.get("input").and_then(|i| i.as_str()) {
            return Some(input.to_string());
        }

        // Claude format
        if let Some(tool_input) = tool_input.get("tool_input").and_then(|i| i.as_str()) {
            return Some(tool_input.to_string());
        }

        // Generic format - try common fields
        for field in ["command", "cmd", "input", "tool_input"] {
            if let Some(value) = tool_input.get(field) {
                if let Some(s) = value.as_str() {
                    return Some(s.to_string());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, CustomRule, ExecutionMode, RuleSeverity};
    use crate::hooks::{EventContext, EventType, ProjectLintEvent};
    use serde_json::json;
    use std::path::PathBuf;

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
            disabled_if_path_exists: None,
            enabled_if_path_exists: None,
            exclude_patterns: vec![],
            triggers: vec!["pre_tool_use".to_string()],
            mode: ExecutionMode::LocalSync,
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
                cwd: None,
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
        let config = Config::default();
        let engine = RuleEngine::new(&config);

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
}
