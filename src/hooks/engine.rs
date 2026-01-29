use crate::hooks::{ProjectLintEvent, HookResult, Decision};
use crate::config::{Config, CustomRule, ModularRule, RuleSeverity};
use crate::utils::Result;
use std::path::Path;
use std::fs;
use serde_json;
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
            let has_errors = issues.iter().any(|i| matches!(i.severity, RuleSeverity::Error));

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
                    if let Some(event) = event {
                        if let Some(tool_input) = &event.context.tool_input {
                            if let Some(command_str) = self.extract_command_from_input(tool_input) {
                                if command_str.starts_with("npm ") {
                                    let rewritten_command = command_str.replace("npm ", "pnpm ");
                                    // Create modified input with the rewritten command
                                    if let Some(input_field) = tool_input.get("input") {
                                        let mut new_input = tool_input.clone();
                                        new_input["input"] = serde_json::Value::String(rewritten_command);
                                        modified_input = Some(new_input);
                                    } else if let Some(tool_input_field) = tool_input.get("tool_input") {
                                        let mut new_input = tool_input.clone();
                                        new_input["tool_input"] = serde_json::Value::String(rewritten_command);
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

    fn evaluate_custom_rule(&self, rule: &CustomRule, event: &ProjectLintEvent) -> Result<Option<DetectedIssue>> {
        // For event hooks, we mainly check context like file path or prompt content
        let mut matched = false;

        // Check if rule mentions a pattern that matches event file path
        if let Some(file_path) = &event.context.file_path {
            let path_str = file_path.to_string_lossy();
            if crate::utils::matches_pattern(&path_str, &rule.pattern) {
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
    fn evaluate_pnpm_rule(&self, rule: &CustomRule, event: &ProjectLintEvent) -> Result<Option<DetectedIssue>> {
        // Only check on PreToolUse events
        if event.event_type != crate::hooks::EventType::PreToolUse {
            return Ok(None);
        }

        // Get the current working directory
        let cwd = event.cwd.as_ref()
            .or_else(|| std::env::current_dir().ok())
            .map(|p| p.as_path())
            .unwrap_or(Path::new("."));

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
                || project_path.join("pnpm-workspace.yml").exists() {
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
    // Import test module from separate file
    mod engine_tests;
}
