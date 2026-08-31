use crate::config::{Config, CustomRule, ModularRule, RuleSeverity};
use crate::hooks::{Decision, EventType, HookResult, ProjectLintEvent};
use crate::utils::{matches_pattern, path_exists_glob, Result};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

        // Special handling for worktree isolation enforcement.
        // Blocks writes/subagent dispatch and stop-with-dirty-tree when the
        // current branch is a protected branch (main/master) AND the cwd is
        // the main worktree (not a linked `git worktree add` worktree).
        if rule.name == "worktree-isolation-enforcer" {
            return self.evaluate_worktree_isolation_rule(rule, event);
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

    /// Evaluate worktree isolation enforcement.
    ///
    /// Prevents direct edits, subagent dispatch, and stop-with-dirty-tree on
    /// protected branches (main/master) when running in the **main worktree**
    /// (i.e. not inside a linked `git worktree add` worktree). This codifies
    /// the "all work on main happens in a worktree" practice:
    ///
    /// - **PreToolUse** for write/edit tools (`Edit`, `Write`, `MultiEdit`,
    ///   `NotebookEdit`) and subagent dispatch (`Task`, `run_subagent`):
    ///   denied on protected branch in main worktree. This closes the gap
    ///   where only subagent dispatch was blocked — direct edits are now
    ///   blocked too.
    /// - **PostToolUse**: re-runs the protected_paths + branch check after a
    ///   write lands, catching writes that slipped through (e.g. hook
    ///   bypassed with `--no-verify`, or a tool the matcher missed).
    /// - **Stop**: denied when the protected branch in the main worktree has
    ///   a dirty working tree (uncommitted changes). This fires on **every**
    ///   Stop event — there is no once-per-session suppression, so a
    ///   recovered-but-still-dirty state will re-trigger the guard.
    /// - **SubagentStop**: same dirty-tree guard as Stop, but fires as soon
    ///   as a subagent returns rather than waiting for the top-level Stop.
    ///
    /// All checks are no-ops inside a linked worktree (where work on main is
    /// expected to happen) and on non-protected branches. The protected
    /// branch list is configurable via `rule.protected_branches`; when empty,
    /// it defaults to `["main", "master", "trunk", "develop"]`.
    fn evaluate_worktree_isolation_rule(
        &self,
        rule: &CustomRule,
        event: &ProjectLintEvent,
    ) -> Result<Option<DetectedIssue>> {
        // Act on PreToolUse (gate before write/subagent), PostToolUse
        // (verify after write), Stop (dirty-tree guard), and SubagentStop
        // (dirty-tree guard when a subagent returns).
        match event.event_type {
            EventType::PreToolUse
            | EventType::PostToolUse
            | EventType::Stop
            | EventType::SubagentStop => {}
            _ => return Ok(None),
        }

        let cwd_buf = event
            .cwd
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let cwd = cwd_buf.as_path();

        // Only enforce inside a git repo.
        if !is_inside_git_repo(cwd)? {
            return Ok(None);
        }

        // Only enforce on protected branches. When the rule doesn't specify
        // a list, fall back to the conventional defaults.
        let branch = match current_branch(cwd)? {
            Some(b) => b,
            None => return Ok(None), // detached HEAD — let git hooks handle it
        };
        let configured: Vec<&str> = rule.protected_branches.iter().map(|s| s.as_str()).collect();
        let protected: &[&str] = if configured.is_empty() {
            &DEFAULT_PROTECTED_BRANCHES[..]
        } else {
            &configured[..]
        };
        if !is_protected_branch(&branch, protected) {
            return Ok(None);
        }

        // Inside a linked worktree, work on main is allowed.
        if is_in_linked_worktree(cwd)? {
            return Ok(None);
        }

        match event.event_type {
            EventType::PreToolUse | EventType::PostToolUse => {
                let tool = event.context.tool_name.as_deref().unwrap_or("");

                // Subagent dispatch is never scoped by paths — a subagent
                // inherits the cwd and could touch anything in the tree, so
                // it is always blocked on a protected branch in the main
                // worktree.
                if is_subagent_tool(tool) {
                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: format!(
                            "🚫 Worktree isolation: subagent dispatch on protected branch \
                             '{branch}' is blocked outside a linked worktree.\n\n\
                             Create a worktree before working on {branch}:\n  \
                             git worktree add ../{branch}-work -b work/{branch}\n\
                             Then re-run your command from inside that worktree."
                        ),
                        severity: rule.severity.clone(),
                    }));
                }

                // Write/edit tools are scoped by `protected_paths`. When the
                // list is empty, default to `src/**` (protect source code,
                // allow docs/config edits on a protected branch).
                if is_write_tool(tool) {
                    let globs = if rule.protected_paths.is_empty() {
                        &["src/**".to_string()][..]
                    } else {
                        &rule.protected_paths[..]
                    };

                    let file_path = resolve_write_file_path(event);
                    let in_scope = match file_path.as_deref() {
                        Some(p) => path_matches_any_glob(p, globs),
                        // Can't determine the target path — don't block,
                        // since we can't confirm it's under a protected path.
                        None => false,
                    };

                    if in_scope {
                        // PostToolUse fires *after* the write landed on disk.
                        // Verify the file actually changed via `git status` —
                        // a no-op write (same content) is not a violation.
                        // PreToolUse always blocks since it fires *before*
                        // the write and can't know yet whether it'll change
                        // anything.
                        if event.event_type == EventType::PostToolUse {
                            let changed = match file_path.as_deref() {
                                Some(p) => is_path_dirty(cwd, p)?,
                                None => false,
                            };
                            if !changed {
                                return Ok(None);
                            }
                        }

                        let label = if event.event_type == EventType::PostToolUse {
                            "write detected on"
                        } else {
                            "direct edit on"
                        };
                        return Ok(Some(DetectedIssue {
                            name: rule.name.clone(),
                            message: format!(
                                "🚫 Worktree isolation: {label} protected branch \
                                 '{branch}' is blocked outside a linked worktree.\n\n\
                                 Create a worktree before working on {branch}:\n  \
                                 git worktree add ../{branch}-work -b work/{branch}\n\
                                 Then re-run your command from inside that worktree."
                            ),
                            severity: rule.severity.clone(),
                        }));
                    }
                }
                Ok(None)
            }
            EventType::Stop | EventType::SubagentStop => {
                // Block stopping with a dirty protected branch in the main
                // worktree. SubagentStop fires when a subagent returns — the
                // guard catches a dirty tree as soon as the subagent finishes,
                // not after the whole session.
                if is_dirty(cwd)? {
                    let label = if event.event_type == EventType::SubagentStop {
                        "subagent stop on"
                    } else {
                        "stopping on"
                    };
                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: format!(
                            "🚫 Worktree isolation: {label} protected branch '{branch}' \
                             with uncommitted changes in the main worktree is blocked.\n\n\
                             Either:\n  \
                             1. Move your work into a linked worktree:\n     \
                             git worktree add ../{branch}-work -b work/{branch}\n  \
                             2. Or commit/stash your changes before stopping.\n\n\
                             This guard fires on every Stop/SubagentStop event — it is \
                             not suppressed after the first attempt."
                        ),
                        severity: rule.severity.clone(),
                    }));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
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

// ---------------------------------------------------------------------------
// Worktree isolation helpers
// ---------------------------------------------------------------------------

/// Tool names that perform direct file edits. Blocking these on a protected
/// branch in the main worktree closes the gap where only subagent dispatch
/// was blocked.
fn is_write_or_subagent_tool(tool: &str) -> bool {
    is_write_tool(tool) || is_subagent_tool(tool)
}

fn is_write_tool(tool: &str) -> bool {
    matches!(
        tool,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "edit" | "write"
    )
}

/// Tool names that dispatch a subagent (which inherits the cwd and would
/// operate on the main worktree). Covers Claude Code's `Task` tool and the
/// generic `run_subagent` name used by other agents.
fn is_subagent_tool(tool: &str) -> bool {
    matches!(
        tool,
        "Task" | "run_subagent" | "RunSubagent" | "subagent" | "launch_subagent"
    )
}

/// Conventional protected branches used when a rule doesn't specify its own
/// list. Kept as a static slice so the evaluator can borrow it without
/// allocating.
static DEFAULT_PROTECTED_BRANCHES: [&str; 4] = ["main", "master", "trunk", "develop"];

/// Branches where direct work is forbidden outside a linked worktree.
/// `protected` is the configured list (or `DEFAULT_PROTECTED_BRANCHES`).
fn is_protected_branch(branch: &str, protected: &[&str]) -> bool {
    protected.contains(&branch)
}

/// Resolve the target file path of a write/edit tool event.
///
/// The Claude mapper populates `context.file_path` for `Read`/`Edit`/`Write`
/// but not for `MultiEdit`/`NotebookEdit`, so we also peek at `tool_input`
/// (`file_path`, `notebook_path`, `path`) as a fallback. Returns the path as
/// a string (absolute or repo-relative — glob matching handles both).
fn resolve_write_file_path(event: &ProjectLintEvent) -> Option<String> {
    if let Some(p) = &event.context.file_path {
        return Some(p.to_string_lossy().to_string());
    }
    if let Some(input) = &event.context.tool_input {
        for field in ["file_path", "notebook_path", "path"] {
            if let Some(s) = input.get(field).and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// True if `path` matches any of the glob patterns. Falls back to a simple
/// `matches_pattern` substring/suffix check for non-glob patterns.
fn path_matches_any_glob(path: &str, globs: &[String]) -> bool {
    for g in globs {
        if crate::utils::is_glob(g) {
            if let Ok(p) = glob::Pattern::new(g) {
                if p.matches(path) {
                    return true;
                }
            }
        } else if matches_pattern(path, g) {
            return true;
        }
    }
    false
}

/// True if `cwd` is inside a git repository.
fn is_inside_git_repo(cwd: &Path) -> Result<bool> {
    Ok(git_run(cwd, &["rev-parse", "--is-inside-work-tree"])?.trim() == "true")
}

/// The current branch name, or `None` for detached HEAD.
fn current_branch(cwd: &Path) -> Result<Option<String>> {
    let out = git_run(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let s = out.trim();
    if s.is_empty() || s == "HEAD" {
        Ok(None)
    } else {
        Ok(Some(s.to_string()))
    }
}

/// True if `cwd` is a **linked** worktree (created via `git worktree add`),
/// as opposed to the main worktree.
///
/// In the main worktree, `--git-dir` and `--git-common-dir` resolve to the
/// same path. In a linked worktree, `--git-dir` points at
/// `<common>/.git/worktrees/<name>` while `--git-common-dir` points at the
/// shared `<common>/.git`, so they differ.
fn is_in_linked_worktree(cwd: &Path) -> Result<bool> {
    let git_dir = git_run(cwd, &["rev-parse", "--git-dir"])?;
    let common_dir = git_run(cwd, &["rev-parse", "--git-common-dir"])?;
    let git_dir = normalize_git_path(cwd, git_dir.trim());
    let common_dir = normalize_git_path(cwd, common_dir.trim());
    Ok(git_dir != common_dir)
}

/// True if the working tree has any uncommitted changes (staged or unstaged).
fn is_dirty(cwd: &Path) -> Result<bool> {
    let out = git_run(cwd, &["status", "--porcelain"])?;
    Ok(!out.trim().is_empty())
}

/// True if `path` (relative to `cwd`) has uncommitted changes — modified,
/// staged, untracked, or deleted. Used by PostToolUse to verify that a write
/// actually changed the file on disk, rather than no-op'ing with identical
/// content.
///
/// Returns `false` when `path` cannot be resolved or git reports no change.
/// This is the safe default: a no-op write to a protected path on a protected
/// branch in the main worktree is not a violation if the tree is clean.
fn is_path_dirty(cwd: &Path, path: &str) -> Result<bool> {
    // Normalize the target path to a repo-relative form. Absolute paths
    // (common from Claude Code's tool_input) are made relative to cwd so
    // `git status -- <path>` matches against the worktree.
    let rel = std::path::Path::new(path);
    let rel = if rel.is_absolute() {
        rel.strip_prefix(cwd)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| rel.to_path_buf())
    } else {
        rel.to_path_buf()
    };
    let rel_str = rel.to_string_lossy();
    let out = git_run(cwd, &["status", "--porcelain", "--", rel_str.as_ref()])?;
    Ok(!out.trim().is_empty())
}

/// Resolve a possibly-relative git path against `cwd` to an absolute path for
/// comparison. `git rev-parse --git-dir` returns `.git` in the main worktree
/// and an absolute path in linked worktrees; `--git-common-dir` may be
/// relative or absolute depending on git version.
fn normalize_git_path(cwd: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path.canonicalize().unwrap_or(path)
    } else {
        let joined = cwd.join(&path);
        joined.canonicalize().unwrap_or(joined)
    }
}

/// Run a git command in `cwd` and return its stdout. Returns an empty string
/// on any failure (treated as "not in a repo" / "no branch" by callers).
///
/// Scrubs inherited `GIT_*` environment variables (e.g. `GIT_INDEX_FILE`,
/// `GIT_DIR`) that git hooks set — without this, git commands run from inside
/// a hook would operate on the hook's repo/index instead of the target cwd,
/// causing "index file open failed: Not a directory" errors in tests that
/// create linked worktrees.
fn git_run(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(clean_env())
        .output();
    match output {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).to_string()),
        Ok(o) => {
            debug!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&o.stderr).trim()
            );
            Ok(String::new())
        }
        Err(e) => {
            debug!("git {} could not start: {}", args.join(" "), e);
            Ok(String::new())
        }
    }
}

/// Minimal environment for git subprocesses: PATH so git is found, plus HOME
/// for config. All `GIT_*` vars are excluded so hook-inherited state doesn't
/// leak into the subprocess.
fn clean_env() -> Vec<(String, String)> {
    let mut env = Vec::new();
    for key in ["PATH", "HOME", "USER", "TMPDIR", "LANG", "LC_ALL"] {
        if let Ok(val) = std::env::var(key) {
            env.push((key.to_string(), val));
        }
    }
    env
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
            protected_paths: vec![],
            protected_branches: vec![],
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
            protected_paths: vec![],
            protected_branches: vec![],
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
            protected_paths: vec![],
            protected_branches: vec![],
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

    // ── worktree isolation enforcement ──

    fn make_worktree_config() -> Config {
        let mut config = Config::default();
        config.rules.custom_rules.push(CustomRule {
            name: "worktree-isolation-enforcer".to_string(),
            pattern: "*".to_string(),
            message: "worktree isolation".to_string(),
            severity: RuleSeverity::Error,
            check_content: false,
            content_pattern: None,
            exception_pattern: None,
            condition: None,
            required: false,
            required_if_path_exists: None,
            disabled_if_path_exists: None,
            enabled_if_path_exists: None,
            exclude_patterns: vec![],
            protected_paths: vec!["src/**".to_string()],
            protected_branches: vec![],
            triggers: vec![
                "pre_tool_use".to_string(),
                "post_tool_use".to_string(),
                "stop".to_string(),
                "subagent_stop".to_string(),
            ],
            mode: ExecutionMode::LocalSync,
        });
        config
    }

    /// Create a temp git repo with an initial commit on `main` (or the
    /// default initial branch). Returns the TempDir (keep it alive for the
    /// test) and the path to use as cwd.
    fn make_git_repo_on_main() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path();
        // init with an explicit branch name so the test is deterministic
        // regardless of the user's global init.defaultBranch setting.
        // Use -b (supported since git 2.28); --branch is not a valid flag.
        run_git(path, &["init", "-b", "main"]);
        run_git(path, &["config", "user.email", "t@t.test"]);
        run_git(path, &["config", "user.name", "test"]);
        std::fs::write(path.join("README.md"), "init\n").unwrap();
        run_git(path, &["add", "README.md"]);
        run_git(path, &["commit", "-m", "init"]);
        dir
    }

    fn run_git(cwd: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(clean_env())
            .output()
            .expect("git runs");
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    #[test]
    fn test_worktree_helpers_classification() {
        assert!(is_write_tool("Edit"));
        assert!(is_write_tool("Write"));
        assert!(is_write_tool("MultiEdit"));
        assert!(is_write_tool("NotebookEdit"));
        assert!(!is_write_tool("Read"));
        assert!(is_subagent_tool("Task"));
        assert!(is_subagent_tool("run_subagent"));
        assert!(!is_subagent_tool("Edit"));
        assert!(is_write_or_subagent_tool("Edit"));
        assert!(is_write_or_subagent_tool("Task"));
        assert!(!is_write_or_subagent_tool("Read"));
        let defaults = &DEFAULT_PROTECTED_BRANCHES[..];
        assert!(is_protected_branch("main", defaults));
        assert!(is_protected_branch("master", defaults));
        assert!(is_protected_branch("develop", defaults));
        assert!(!is_protected_branch("feature/x", defaults));
        // Configurable list: a custom branch like "release/*" is protected
        // when explicitly listed, and "main" is not when omitted.
        let custom = vec!["release/prod", "production"];
        assert!(is_protected_branch("release/prod", &custom));
        assert!(is_protected_branch("production", &custom));
        assert!(!is_protected_branch("main", &custom));
    }

    #[test]
    fn test_worktree_isolation_blocks_edit_on_main() {
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Deny);
        let msg = result.message.unwrap();
        assert!(msg.contains("direct edit"), "msg: {msg}");
        assert!(msg.contains("main"));
    }

    #[test]
    fn test_worktree_isolation_allows_docs_write_on_main() {
        // protected_paths = ["src/**"] → a write to docs/ on main is allowed.
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Write".to_string()),
                tool_input: Some(json!({ "file_path": "docs/guide.md" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(
            result.decision,
            Decision::Allow,
            "writes outside src/ on main must be allowed under protected_paths scoping"
        );
        assert!(result.message.is_none());
    }

    #[test]
    fn test_worktree_isolation_blocks_multiedit_in_src_via_tool_input() {
        // MultiEdit isn't mapped to context.file_path by the Claude mapper,
        // so the evaluator must read file_path from tool_input.
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("MultiEdit".to_string()),
                tool_input: Some(json!({ "file_path": "src/main.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Deny);
    }

    #[test]
    fn test_worktree_isolation_default_src_when_protected_paths_empty() {
        // Empty protected_paths defaults to ["src/**"].
        let dir = make_git_repo_on_main();
        let mut config = make_worktree_config();
        config.rules.custom_rules[0].protected_paths = vec![];
        let engine = RuleEngine::new(&config);

        let src_event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };
        let docs_event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "README.md" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        assert_eq!(
            engine.evaluate_event(&src_event).unwrap().decision,
            Decision::Deny,
            "empty protected_paths defaults to src/**"
        );
        assert_eq!(
            engine.evaluate_event(&docs_event).unwrap().decision,
            Decision::Allow,
            "empty protected_paths still allows non-src writes"
        );
    }

    #[test]
    fn test_worktree_isolation_blocks_subagent_on_main() {
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Task".to_string()),
                tool_input: Some(json!({})),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Deny);
        let msg = result.message.unwrap();
        assert!(msg.contains("subagent dispatch"), "msg: {msg}");
    }

    #[test]
    fn test_worktree_isolation_allows_read_on_main() {
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Read".to_string()),
                tool_input: Some(json!({ "file_path": "README.md" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Allow);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_worktree_isolation_allows_edit_on_feature_branch() {
        let dir = make_git_repo_on_main();
        let path = dir.path();
        run_git(path, &["checkout", "-b", "feature/work"]);
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(path.to_path_buf()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Allow);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_worktree_isolation_stop_blocks_dirty_main() {
        let dir = make_git_repo_on_main();
        let path = dir.path();
        // Make the tree dirty.
        std::fs::write(path.join("dirty.txt"), "uncommitted\n").unwrap();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::Stop,
            session_id: None,
            timestamp: None,
            cwd: Some(path.to_path_buf()),
            context: EventContext {
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Deny);
        let msg = result.message.unwrap();
        assert!(msg.contains("stopping"), "msg: {msg}");
        assert!(msg.contains("not suppressed"));
    }

    #[test]
    fn test_worktree_isolation_stop_allows_clean_main() {
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::Stop,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Allow);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_worktree_isolation_stop_fires_again_on_second_dirty_stop() {
        // No once-per-session suppression: a second Stop event on a dirty
        // main worktree must still be denied.
        let dir = make_git_repo_on_main();
        let path = dir.path();
        std::fs::write(path.join("dirty.txt"), "uncommitted\n").unwrap();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let mk_stop = || ProjectLintEvent {
            event_type: EventType::Stop,
            session_id: Some("sess-1".to_string()),
            timestamp: None,
            cwd: Some(path.to_path_buf()),
            context: EventContext {
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let r1 = engine.evaluate_event(&mk_stop()).unwrap();
        let r2 = engine.evaluate_event(&mk_stop()).unwrap();
        assert_eq!(r1.decision, Decision::Deny);
        assert_eq!(r2.decision, Decision::Deny, "Stop guard must not suppress");
    }

    #[test]
    fn test_worktree_isolation_allows_edit_in_linked_worktree() {
        let dir = make_git_repo_on_main();
        let main_path = dir.path().to_path_buf();
        // Create the linked worktree as a subdirectory INSIDE the main repo
        // temp dir. Git supports worktrees inside the main worktree, and this
        // avoids any race with /tmp sibling dirs or pre-created parents. The
        // path must not exist yet (git creates it).
        let work_cwd = main_path.join("linked-wt");

        let add_out = Command::new("git")
            .args([
                "worktree",
                "add",
                work_cwd.to_str().unwrap(),
                "-b",
                "work/main",
            ])
            .current_dir(&main_path)
            .env_clear()
            .envs(clean_env())
            .output()
            .expect("git worktree add runs");
        assert!(
            add_out.status.success(),
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add_out.stderr)
        );

        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(work_cwd.clone()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(
            result.decision,
            Decision::Allow,
            "edits in a linked worktree on a protected branch must be allowed"
        );
        assert!(result.message.is_none());

        // Cleanup the linked worktree.
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&work_cwd)
            .current_dir(&main_path)
            .env_clear()
            .envs(clean_env())
            .output();
    }

    // ── configurable protected_branches ──

    #[test]
    fn test_worktree_isolation_custom_protected_branches() {
        // A rule with protected_branches = ["release/prod", "production"]
        // should protect those branches but NOT "main".
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path();
        run_git(path, &["init", "-b", "production"]);
        run_git(path, &["config", "user.email", "t@t.test"]);
        run_git(path, &["config", "user.name", "test"]);
        std::fs::write(path.join("README.md"), "init\n").unwrap();
        run_git(path, &["add", "README.md"]);
        run_git(path, &["commit", "-m", "init"]);

        let mut config = Config::default();
        config.rules.custom_rules.push(CustomRule {
            name: "worktree-isolation-enforcer".to_string(),
            pattern: "*".to_string(),
            message: "worktree isolation".to_string(),
            severity: RuleSeverity::Error,
            check_content: false,
            content_pattern: None,
            exception_pattern: None,
            condition: None,
            required: false,
            required_if_path_exists: None,
            disabled_if_path_exists: None,
            enabled_if_path_exists: None,
            exclude_patterns: vec![],
            protected_paths: vec!["src/**".to_string()],
            protected_branches: vec!["release/prod".to_string(), "production".to_string()],
            triggers: vec![
                "pre_tool_use".to_string(),
                "post_tool_use".to_string(),
                "stop".to_string(),
                "subagent_stop".to_string(),
            ],
            mode: ExecutionMode::LocalSync,
        });
        let engine = RuleEngine::new(&config);

        // Edit on "production" (custom-protected) must be blocked.
        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(path.to_path_buf()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };
        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(
            result.decision,
            Decision::Deny,
            "custom-protected branch 'production' must block edits"
        );

        // Now switch to "main" — NOT in the custom list, so edits are allowed.
        run_git(path, &["checkout", "-b", "main"]);
        let event_main = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(path.to_path_buf()),
            context: EventContext {
                tool_name: Some("Edit".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };
        let result_main = engine.evaluate_event(&event_main).unwrap();
        assert_eq!(
            result_main.decision,
            Decision::Allow,
            "main is not in the custom protected_branches list, so edits must be allowed"
        );
    }

    // ── SubagentStop dirty-tree guard ──

    #[test]
    fn test_worktree_isolation_subagent_stop_blocks_dirty_main() {
        let dir = make_git_repo_on_main();
        let path = dir.path();
        std::fs::write(path.join("dirty.txt"), "uncommitted\n").unwrap();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::SubagentStop,
            session_id: None,
            timestamp: None,
            cwd: Some(path.to_path_buf()),
            context: EventContext {
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Deny);
        let msg = result.message.unwrap();
        assert!(msg.contains("subagent stop"), "msg: {msg}");
        assert!(msg.contains("main"), "msg should mention branch: {msg}");
    }

    #[test]
    fn test_worktree_isolation_subagent_stop_allows_clean_main() {
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::SubagentStop,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Allow);
        assert!(result.message.is_none());
    }

    // ── PostToolUse write verification ──

    #[test]
    fn test_worktree_isolation_post_tool_use_catches_write_on_main() {
        // PostToolUse should catch a write that slipped through to src/ on
        // main — but only if the file actually changed on disk. We simulate
        // the write landing by creating the file before evaluating the event.
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        // Simulate the write: create src/lib.rs with new content (the repo
        // only has README.md committed, so this is a new untracked file).
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn new() {}\n").unwrap();

        let event = ProjectLintEvent {
            event_type: EventType::PostToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Write".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Deny);
        let msg = result.message.unwrap();
        assert!(msg.contains("write detected"), "msg: {msg}");
    }

    #[test]
    fn test_worktree_isolation_post_tool_use_allows_noop_write_on_main() {
        // PostToolUse for a write that did not actually change the file
        // (same content) must be allowed — `git status` reports clean.
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        // Commit a tracked file under src/ so we can "write" the same
        // content back and verify git sees no change.
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let content = "pub fn existing() {}\n";
        std::fs::write(dir.path().join("src/lib.rs"), content).unwrap();
        run_git(dir.path(), &["add", "src/lib.rs"]);
        run_git(dir.path(), &["commit", "-m", "add src/lib.rs"]);

        // Simulate a no-op write: same content, file on disk is unchanged.
        // git status -- src/lib.rs reports nothing.
        let event = ProjectLintEvent {
            event_type: EventType::PostToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Write".to_string()),
                tool_input: Some(json!({ "file_path": "src/lib.rs" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(
            result.decision,
            Decision::Allow,
            "a no-op write (same content) must not trigger PostToolUse"
        );
        assert!(result.message.is_none());
    }

    #[test]
    fn test_worktree_isolation_post_tool_use_allows_docs_on_main() {
        // PostToolUse for a docs/ write on main is allowed (not in protected_paths).
        let dir = make_git_repo_on_main();
        let config = make_worktree_config();
        let engine = RuleEngine::new(&config);

        let event = ProjectLintEvent {
            event_type: EventType::PostToolUse,
            session_id: None,
            timestamp: None,
            cwd: Some(dir.path().to_path_buf()),
            context: EventContext {
                tool_name: Some("Write".to_string()),
                tool_input: Some(json!({ "file_path": "docs/guide.md" })),
                ide_source: "claude".to_string(),
                ..Default::default()
            },
        };

        let result = engine.evaluate_event(&event).unwrap();
        assert_eq!(result.decision, Decision::Allow);
        assert!(result.message.is_none());
    }
}
