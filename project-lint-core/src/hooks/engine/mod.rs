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

                // Check if this issue includes a command rewrite suggestion.
                // Both pnpm-workspace-enforcer and uv-workspace-enforcer can
                // produce modified_input with a rewritten command.
                if issue.name == "pnpm-workspace-enforcer" || issue.name == "uv-workspace-enforcer"
                {
                    // Extract the command string from either tool_input (PreToolUse)
                    // or context.command (PreRunCommand / Windsurf).
                    let command_str = if let Some(tool_input) = &event.context.tool_input {
                        self.extract_command_from_input(tool_input)
                    } else if let Some(cmd) = &event.context.command {
                        Some(cmd.clone())
                    } else {
                        None
                    };

                    if let Some(command_str) = command_str {
                        // Determine the rewrite based on the rule and command prefix.
                        let rewritten_command = if issue.name == "pnpm-workspace-enforcer" {
                            // JS package manager rewrites in pnpm workspaces
                            if command_str.starts_with("npx ") {
                                Some(command_str.replace("npx ", "pnpm dlx "))
                            } else if command_str.starts_with("npm ") {
                                Some(command_str.replace("npm ", "pnpm "))
                            } else if command_str.starts_with("yarn ") {
                                Some(command_str.replace("yarn ", "pnpm "))
                            } else if command_str.starts_with("bun ") {
                                Some(command_str.replace("bun ", "pnpm "))
                            } else {
                                None
                            }
                        } else {
                            // Python package manager rewrites in uv projects
                            if command_str.starts_with("pipx ") {
                                Some(command_str.replace("pipx ", "uvx "))
                            } else if command_str.starts_with("pip ") {
                                Some(command_str.replace("pip ", "uv pip "))
                            } else {
                                None
                            }
                        };

                        if let Some(rewritten) = rewritten_command {
                            modified_input = self.rewrite_command_in_tool_input(
                                &event.context.tool_input,
                                &rewritten,
                                modified_input,
                            );
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

        // Special handling for uv enforcement
        if rule.name == "uv-workspace-enforcer" {
            return self.evaluate_uv_rule(rule, event);
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
    ///
    /// Rewrites `npm` → `pnpm`, `npx` → `pnpm dlx`, `yarn` → `pnpm`,
    /// and `bun` → `pnpm` in pnpm workspaces.
    /// Handles both `PreToolUse` (Claude, Devin, generic) and
    /// `PreRunCommand` (Windsurf) events.
    fn evaluate_pnpm_rule(
        &self,
        rule: &CustomRule,
        event: &ProjectLintEvent,
    ) -> Result<Option<DetectedIssue>> {
        // Only check on PreToolUse or PreRunCommand events
        if event.event_type != crate::hooks::EventType::PreToolUse
            && event.event_type != crate::hooks::EventType::PreRunCommand
        {
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

        let command_str = self.extract_command_from_event(event);

        if let Some(command_str) = command_str {
            // Define all rewrites: (prefix, replacement, tool_name)
            let rewrites: &[(&str, &str, &str)] = &[
                ("npx ", "pnpm dlx ", "pnpm dlx"),
                ("npm ", "pnpm ", "pnpm"),
                ("yarn ", "pnpm ", "pnpm"),
                ("bun ", "pnpm ", "pnpm"),
            ];

            for (prefix, replacement, tool_name) in rewrites {
                if command_str.starts_with(prefix) {
                    info!(
                        "Detected {} command in pnpm workspace: {}",
                        prefix.trim(),
                        command_str
                    );

                    let rewritten_command = command_str.replacen(prefix, replacement, 1);

                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: format!(
                            "🚫 This project uses pnpm (detected in package.json).\n\nFound: {}\nSuggested: {}\n\nThe command has been automatically rewritten to use {}.",
                            command_str, rewritten_command, tool_name
                        ),
                        severity: rule.severity.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Evaluate uv enforcement rule
    ///
    /// Rewrites `pip` → `uv pip` and `pipx` → `uvx` by default.
    /// Only skips when the project explicitly uses a different Python
    /// package manager (requirements.txt, Pipfile, poetry.lock, etc.).
    /// Handles both `PreToolUse` (Claude, Devin, generic) and
    /// `PreRunCommand` (Windsurf) events.
    fn evaluate_uv_rule(
        &self,
        rule: &CustomRule,
        event: &ProjectLintEvent,
    ) -> Result<Option<DetectedIssue>> {
        // Only check on PreToolUse or PreRunCommand events
        if event.event_type != crate::hooks::EventType::PreToolUse
            && event.event_type != crate::hooks::EventType::PreRunCommand
        {
            return Ok(None);
        }

        // Get the current working directory
        let cwd_buf = event
            .cwd
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let cwd = cwd_buf.as_path();

        // uv is the default — only skip if the project opts out
        if !self.should_enforce_uv(cwd)? {
            debug!("Project opts out of uv enforcement, skipping");
            return Ok(None);
        }

        let command_str = self.extract_command_from_event(event);

        if let Some(command_str) = command_str {
            // Define all rewrites: (prefix, replacement, tool_name)
            let rewrites: &[(&str, &str, &str)] =
                &[("pipx ", "uvx ", "uvx"), ("pip ", "uv pip ", "uv pip")];

            for (prefix, replacement, tool_name) in rewrites {
                if command_str.starts_with(prefix) {
                    info!(
                        "Detected {} command, rewriting to {}: {}",
                        prefix.trim(),
                        tool_name,
                        command_str
                    );

                    let rewritten_command = command_str.replacen(prefix, replacement, 1);

                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: format!(
                            "🚫 uv is the default package manager.\n\nFound: {}\nSuggested: {}\n\nThe command has been automatically rewritten to use {}.",
                            command_str, rewritten_command, tool_name
                        ),
                        severity: rule.severity.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Extract the command string from an event, checking both tool_input
    /// (PreToolUse) and context.command (PreRunCommand / Windsurf).
    fn extract_command_from_event(&self, event: &ProjectLintEvent) -> Option<String> {
        if let Some(tool_input) = &event.context.tool_input {
            self.extract_command_from_input(tool_input)
        } else if let Some(cmd) = &event.context.command {
            Some(cmd.clone())
        } else {
            None
        }
    }

    /// Rewrite a command string inside a tool_input JSON value, producing
    /// a modified_input suitable for the HookResult. Handles different IDE
    /// field formats (input, tool_input, command, cmd).
    fn rewrite_command_in_tool_input(
        &self,
        tool_input: &Option<serde_json::Value>,
        rewritten: &str,
        existing: Option<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        // If we already have a modified_input from a previous issue, keep it.
        if existing.is_some() {
            return existing;
        }

        if let Some(tool_input) = tool_input {
            // Try known field names in priority order
            for field in ["input", "tool_input", "command", "cmd"] {
                if tool_input.get(field).is_some() {
                    let mut new_input = tool_input.clone();
                    new_input[field] = serde_json::Value::String(rewritten.to_string());
                    return Some(new_input);
                }
            }
        }

        None
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

    /// Determine whether uv enforcement should be active.
    ///
    /// uv is the **default** — pip/pipx are rewritten to `uv pip`/`uvx`
    /// unless the project explicitly uses a different Python package manager.
    ///
    /// Opt-out markers (any one of these disables enforcement):
    /// - `requirements.txt` without `uv.lock` — traditional pip project
    /// - `Pipfile` or `Pipfile.lock` — pipenv project
    /// - `poetry.lock` — poetry project
    /// - `setup.py` without `pyproject.toml` — legacy setuptools project
    ///
    /// If `uv.lock` or `[tool.uv]` in `pyproject.toml` is present, enforcement
    /// is always active regardless of other markers.
    fn should_enforce_uv(&self, project_path: &Path) -> Result<bool> {
        // Explicit opt-in: uv.lock or [tool.uv] always wins
        if project_path.join("uv.lock").exists() {
            return Ok(true);
        }

        let pyproject_path = project_path.join("pyproject.toml");
        if pyproject_path.exists() {
            let content = fs::read_to_string(&pyproject_path)?;
            if content.contains("[tool.uv]") {
                return Ok(true);
            }
        }

        // Opt-out: traditional pip project (requirements.txt without uv.lock)
        if project_path.join("requirements.txt").exists() {
            debug!("requirements.txt present without uv.lock, skipping uv enforcement");
            return Ok(false);
        }

        // Opt-out: pipenv project
        if project_path.join("Pipfile").exists() || project_path.join("Pipfile.lock").exists() {
            debug!("Pipfile present, skipping uv enforcement");
            return Ok(false);
        }

        // Opt-out: poetry project
        if project_path.join("poetry.lock").exists() {
            debug!("poetry.lock present, skipping uv enforcement");
            return Ok(false);
        }

        // Opt-out: legacy setuptools project (setup.py without pyproject.toml)
        if project_path.join("setup.py").exists() && !pyproject_path.exists() {
            debug!("setup.py without pyproject.toml, skipping uv enforcement");
            return Ok(false);
        }

        // Default: enforce uv
        Ok(true)
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

    #[test]
    fn test_npx_command_extraction() {
        let config = Config::default();
        let engine = RuleEngine::new(&config);

        // Devin CLI exec tool uses "command" field
        let devin_input = json!({
            "command": "npx create-react-app my-app"
        });
        assert_eq!(
            engine.extract_command_from_input(&devin_input),
            Some("npx create-react-app my-app".to_string())
        );

        // Windsurf format with npx
        let windsurf_input = json!({
            "input": "npx prettier --write ."
        });
        assert_eq!(
            engine.extract_command_from_input(&windsurf_input),
            Some("npx prettier --write .".to_string())
        );
    }

    /// Helper: create a pnpm rule config
    fn make_pnpm_config() -> Config {
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
            triggers: vec!["pre_tool_use".to_string(), "pre_run_command".to_string()],
            mode: ExecutionMode::LocalSync,
        });
        config
    }

    /// Helper: create a temp pnpm workspace dir
    fn make_pnpm_workspace() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let package_json = dir.path().join("package.json");
        std::fs::write(
            &package_json,
            r#"{"name": "test", "packageManager": "pnpm@9.0.0"}"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_npx_rewrite_to_pnpm_dlx_in_pnpm_workspace() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "npx create-react-app my-app"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("pnpm dlx"));
        assert!(msg.contains("npx create-react-app"));
        assert!(msg.contains("pnpm dlx create-react-app"));

        // Verify modified_input contains the rewritten command
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(
            modified["command"],
            json!("pnpm dlx create-react-app my-app")
        );
    }

    #[test]
    fn test_npm_rewrite_to_pnpm_in_pnpm_workspace() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "npm install express"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("pnpm install express"));

        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("pnpm install express"));
    }

    #[test]
    fn test_npx_rewrite_windsurf_format() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        // Windsurf uses "input" field in tool_input
        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("bash".to_string()),
                tool_input: Some(json!({
                    "input": "npx prettier --write ."
                })),
                ide_source: "windsurf".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["input"], json!("pnpm dlx prettier --write ."));
    }

    #[test]
    fn test_npx_rewrite_pre_run_command() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        // Windsurf PreRunCommand puts command in context.command
        let event = ProjectLintEvent {
            event_type: EventType::PreRunCommand,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                command: Some("npx tsc --noEmit".to_string()),
                ide_source: "windsurf".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("pnpm dlx tsc --noEmit"));
    }

    #[test]
    fn test_no_rewrite_in_non_pnpm_workspace() {
        let config = make_pnpm_config();
        let dir = tempfile::TempDir::new().unwrap();
        // No package.json → not a pnpm workspace
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "npx create-react-app my-app"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    // ── yarn / bun rewrites in pnpm workspaces ──

    #[test]
    fn test_yarn_rewrite_to_pnpm_in_pnpm_workspace() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "yarn add express"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("pnpm add express"));

        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("pnpm add express"));
    }

    #[test]
    fn test_bun_rewrite_to_pnpm_in_pnpm_workspace() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "bun install"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("pnpm install"));

        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("pnpm install"));
    }

    #[test]
    fn test_yarn_rewrite_windsurf_format() {
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("bash".to_string()),
                tool_input: Some(json!({
                    "input": "yarn run build"
                })),
                ide_source: "windsurf".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["input"], json!("pnpm run build"));
    }

    // ── pip / pipx rewrites in uv projects ──

    /// Helper: create a uv rule config
    fn make_uv_config() -> Config {
        let mut config = Config::default();
        config.rules.custom_rules.push(CustomRule {
            name: "uv-workspace-enforcer".to_string(),
            pattern: "*".to_string(),
            message: "Use uv instead".to_string(),
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
            triggers: vec!["pre_tool_use".to_string(), "pre_run_command".to_string()],
            mode: ExecutionMode::LocalSync,
        });
        config
    }

    /// Helper: create a temp uv project dir with uv.lock
    fn make_uv_project() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("uv.lock"), "").unwrap();
        dir
    }

    /// Helper: create a temp uv project dir with pyproject.toml [tool.uv]
    fn make_uv_project_pyproject() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            r#"[project]
name = "test"
version = "0.1.0"

[tool.uv]
dev-dependencies = ["pytest"]
"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_pip_rewrite_to_uv_pip_in_uv_project() {
        let config = make_uv_config();
        let _project = make_uv_project();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_project.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("uv pip install requests"));

        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("uv pip install requests"));
    }

    #[test]
    fn test_pipx_rewrite_to_uvx_in_uv_project() {
        let config = make_uv_config();
        let _project = make_uv_project();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_project.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pipx run ruff check ."
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("uvx run ruff check ."));

        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("uvx run ruff check ."));
    }

    #[test]
    fn test_pip_rewrite_windsurf_format() {
        let config = make_uv_config();
        let _project = make_uv_project();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_project.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("bash".to_string()),
                tool_input: Some(json!({
                    "input": "pip install -r requirements.txt"
                })),
                ide_source: "windsurf".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(
            modified["input"],
            json!("uv pip install -r requirements.txt")
        );
    }

    #[test]
    fn test_pipx_rewrite_pre_run_command() {
        let config = make_uv_config();
        let _project = make_uv_project();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreRunCommand,
            session_id: None,
            timestamp: None,
            cwd: Some(_project.path().to_path_buf()),
            context: EventContext {
                command: Some("pipx run black .".to_string()),
                ide_source: "windsurf".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("uvx run black ."));
    }

    #[test]
    fn test_uv_detection_via_pyproject_toml() {
        let config = make_uv_config();
        let _project = make_uv_project_pyproject();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_project.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install pytest"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("uv pip install pytest"));
    }

    #[test]
    fn test_pip_rewrite_in_empty_dir_default_enforce() {
        // uv is the default — an empty dir with no Python markers
        // should still rewrite pip → uv pip
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("uv pip install requests"));
    }

    #[test]
    fn test_no_rewrite_with_requirements_txt() {
        // requirements.txt without uv.lock → traditional pip project, opt out
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "requests\n").unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    #[test]
    fn test_rewrite_with_requirements_txt_and_uv_lock() {
        // requirements.txt + uv.lock → uv.lock wins, enforce
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "requests\n").unwrap();
        std::fs::write(dir.path().join("uv.lock"), "").unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("uv pip install requests"));
    }

    #[test]
    fn test_no_rewrite_with_pipfile() {
        // Pipfile → pipenv project, opt out
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Pipfile"), "[packages]\n").unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    #[test]
    fn test_no_rewrite_with_poetry_lock() {
        // poetry.lock → poetry project, opt out
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("poetry.lock"), "").unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    #[test]
    fn test_no_rewrite_with_setup_py_no_pyproject() {
        // setup.py without pyproject.toml → legacy setuptools, opt out
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("setup.py"),
            "from setuptools import setup\n",
        )
        .unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    #[test]
    fn test_rewrite_with_setup_py_and_pyproject() {
        // setup.py + pyproject.toml (without [tool.uv]) → modern project,
        // no opt-out marker → default enforce
        let config = make_uv_config();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("setup.py"),
            "from setuptools import setup\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = 'test'\n",
        )
        .unwrap();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_some());
        let modified = result.modified_input.expect("should have modified input");
        assert_eq!(modified["command"], json!("uv pip install requests"));
    }

    #[test]
    fn test_no_pip_rewrite_in_pnpm_workspace() {
        // pip commands should NOT be rewritten in pnpm workspaces
        // (pnpm rule only handles JS package managers)
        let config = make_pnpm_config();
        let _workspace = make_pnpm_workspace();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_workspace.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "pip install requests"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    #[test]
    fn test_no_npm_rewrite_in_uv_project() {
        // npm commands should NOT be rewritten in uv projects
        // (uv rule only handles Python package managers)
        let config = make_uv_config();
        let _project = make_uv_project();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(_project.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("exec".to_string()),
                tool_input: Some(json!({
                    "command": "npm install express"
                })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }
}
