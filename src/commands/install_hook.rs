use clap::Args;
use project_lint_core::utils::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

#[derive(Args)]
pub struct InstallHookArgs {
    /// Target agent (windsurf, claude, cursor, devin, pi, generic, git-hooks,
    /// all, github, gitlab). Use "all" to install both git hooks and Claude
    /// Code hooks (.claude/settings.json + hook.sh) in one command.
    #[arg(long, default_value = "windsurf")]
    pub agent: String,

    /// Installation directory (defaults to agent's default location)
    #[arg(short, long)]
    pub dir: Option<String>,

    /// Force overwrite existing hooks
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: InstallHookArgs) -> Result<()> {
    info!("Installing project-lint hook for {} agent", args.agent);

    match args.agent.to_lowercase().as_str() {
        "windsurf" => install_windsurf_hook(&args).await?,
        "claude" => install_claude_hook(&args).await?,
        "cursor" => install_cursor_hook(&args).await?,
        "devin" => install_devin_hook(&args).await?,
        "pi" => install_pi_hook(&args).await?,
        "generic" => install_generic_hook(&args).await?,
        "git-hooks" => install_git_hooks(&args).await?,
        // "all" installs git hooks (pre-commit/pre-push with worktree
        // isolation) AND Claude Code hooks (.claude/settings.json + hook.sh
        // registered for PreToolUse/Stop) so a single command wires up both
        // the VCS gate and the in-editor guard.
        "all" => {
            install_git_hooks(&args).await?;
            install_claude_hook(&args).await?;
        }
        "github" => install_github_workflow(&args).await?,
        "gitlab" => install_gitlab_workflow(&args).await?,
        _ => {
            error!("Unsupported agent: {}", args.agent);
            return Err(anyhow::anyhow!("Unsupported agent: {}", args.agent));
        }
    }

    info!("Hook installation completed successfully");
    Ok(())
}

async fn install_windsurf_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".windsurf")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Windsurf hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

{bin_resolution}
HOOK_TYPE="windsurf"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        bin_resolution = shell_bin_resolution_snippet()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    // Create Windsurf configuration
    let config_content = r#"[hooks]
pre_tool_use = "./hook.sh"
post_tool_use = "./hook.sh"
pre_read_code = "./hook.sh"
post_read_code = "./hook.sh"
pre_write_code = "./hook.sh"
post_write_code = "./hook.sh"
"#;

    let config_path = hook_dir.join("config.toml");
    if !config_path.exists() || args.force {
        fs::write(&config_path, config_content)?;
        info!("Created Windsurf hook configuration at {:?}", config_path);
    }

    info!("Windsurf hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_claude_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".claude")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Claude Code hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

{bin_resolution}
HOOK_TYPE="claude"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        bin_resolution = shell_bin_resolution_snippet()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    // Register the hook with Claude Code via .claude/settings.json.
    // Claude Code auto-discovers hooks from this file. We merge our hook
    // entries into any existing settings non-destructively so user-configured
    // permissions and other hooks are preserved.
    let settings_path = hook_dir.join("settings.json");
    let hook_command = ".claude/hook.sh".to_string();
    let merged = merge_claude_settings(&settings_path, &hook_command, args.force)?;
    fs::write(&settings_path, merged)?;
    info!(
        "Claude Code hook settings registered at {:?}",
        settings_path
    );
    info!("Claude Code hook installed at {:?}", hook_path);
    Ok(())
}

/// Build (or merge into) a Claude Code `settings.json` the hook entries that
/// wire `.claude/hook.sh` into the events project-lint evaluates.
///
/// Registered events:
/// - `PreToolUse` matching `Edit|Write|MultiEdit|NotebookEdit|Task` — blocks
///   direct edits and subagent dispatch on protected branches outside a
///   linked worktree (worktree isolation), plus pnpm/uv rewrites.
/// - `PostToolUse` matching `Edit|Write|MultiEdit|NotebookEdit` — re-runs
///   the protected_paths + branch check after a write, catching writes that
///   slipped through (e.g. hook bypassed, or a tool the matcher missed).
/// - `Stop` — blocks stopping with a dirty protected branch in the main
///   worktree.
/// - `SubagentStop` — same dirty-tree guard as Stop, fires when a subagent
///   returns.
///
/// Merging is non-destructive: existing top-level keys (permissions, env,
/// etc.) and pre-existing hook entries are preserved. Our entries are added
/// only if an identical command is not already present. When `force` is set
/// and the file exists but is not valid JSON, it is overwritten.
fn merge_claude_settings(settings_path: &Path, hook_command: &str, force: bool) -> Result<String> {
    use serde_json::{json, Value};

    // The events + matchers we want registered.
    let desired: Vec<(&str, Option<&str>)> = vec![
        ("PreToolUse", Some("Edit|Write|MultiEdit|NotebookEdit|Task")),
        ("PostToolUse", Some("Edit|Write|MultiEdit|NotebookEdit")),
        ("Stop", None),
        ("SubagentStop", None),
    ];

    let mut root: Value = if settings_path.exists() {
        match fs::read_to_string(settings_path) {
            Ok(content) if content.trim().is_empty() => json!({}),
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(v) if v.is_object() => v,
                // Unparseable existing file: only overwrite with --force.
                Ok(_) | Err(_) => {
                    if force {
                        warn!(
                            "Existing settings.json at {:?} is not a JSON object; overwriting (--force)",
                            settings_path
                        );
                        json!({})
                    } else {
                        warn!(
                            "Existing settings.json at {:?} is not a JSON object; skipping settings registration. Use --force to overwrite.",
                            settings_path
                        );
                        return Ok(content);
                    }
                }
            },
            Err(_) => json!({}),
        }
    } else {
        json!({})
    };

    let root_obj = root.as_object_mut().expect("root is an object");
    let hooks_obj = root_obj
        .entry("hooks".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("hooks is an object");

    for (event, matcher) in &desired {
        let entry = hooks_obj
            .entry(event.to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("hooks event is an array");

        // For matcher-bearing events, reuse an existing entry with the same
        // matcher; otherwise append a new one. For events without a matcher
        // (Stop), reuse the first entry if it has no matcher.
        let already_present = entry.iter().any(|e| {
            let m = e.get("matcher").and_then(|v| v.as_str()).unwrap_or("");
            match matcher {
                Some(want) => m == *want && entry_has_command(e, hook_command),
                None => m.is_empty() && entry_has_command(e, hook_command),
            }
        });

        if already_present {
            continue;
        }

        let mut hook_entry = json!({
            "hooks": [
                { "type": "command", "command": hook_command }
            ]
        });
        if let Some(m) = matcher {
            hook_entry["matcher"] = json!(m);
        }
        entry.push(hook_entry);
    }

    Ok(serde_json::to_string_pretty(&root)?)
}

/// True if a Claude Code hook entry already contains the given command.
fn entry_has_command(entry: &serde_json::Value, command: &str) -> bool {
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        for h in hooks {
            if h.get("command").and_then(|c| c.as_str()) == Some(command) {
                return true;
            }
        }
    }
    false
}

async fn install_cursor_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".cursor")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Cursor hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

{bin_resolution}
HOOK_TYPE="cursor"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        bin_resolution = shell_bin_resolution_snippet()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Cursor hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_devin_hook(args: &InstallHookArgs) -> Result<()> {
    // Devin CLI uses a Claude-Code-compatible hooks JSON format, stored in
    // .devin/hooks.v1.json. The runtime semantics differ from Claude Code:
    //   - Env var: DEVIN_PROJECT_DIR (not CLAUDE_PROJECT_DIR)
    //   - No exec form (no `args` field); `command` is always shell form (sh -c)
    // See: https://docs.devin.ai/cli/extensibility/hooks/overview
    let hook_dir = get_hook_dir(&args.dir, ".devin")?;
    fs::create_dir_all(&hook_dir)?;

    let project_root = hook_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine project root from hook dir"))?;
    let hook_command = resolve_hook_command(project_root, "DEVIN_PROJECT_DIR")?;

    // Create the hooks.v1.json file with PreToolUse/PostToolUse hooks for exec tool
    let hooks_content = format!(
        r#"{{
  "PreToolUse": [
    {{
      "matcher": "exec",
      "hooks": [
        {{
          "type": "command",
          "command": "{cmd}"
        }}
      ]
    }}
  ],
  "PostToolUse": [
    {{
      "matcher": "exec",
      "hooks": [
        {{
          "type": "command",
          "command": "{cmd}"
        }}
      ]
    }}
  ]
}}
"#,
        cmd = hook_command
    );

    let hooks_path = hook_dir.join("hooks.v1.json");
    write_hook_file(&hooks_path, &hooks_content, args.force)?;

    info!("Devin CLI hook installed at {:?}", hooks_path);
    info!("Devin CLI reads hooks from .devin/hooks.v1.json automatically.");
    info!("Use /hooks in Devin CLI to verify the hook is loaded.");
    Ok(())
}

/// Resolve the hook command string for a generated JSON hook config.
///
/// Used by installers that emit JSON config files (Devin CLI) where the
/// `command` field is a single shell-form string. If the currently running
/// binary is inside `<project_root>/target/{release,debug}/`, emits a
/// `$<project_dir_var>/target/...` path so the hook resolves the binary
/// relative to the project root on any clone (no absolute home path leak).
/// Otherwise, emits a bare `project-lint` command that relies on `$PATH`
/// lookup (cargo install, brew, devbox, etc.).
///
/// The `project_dir_var` parameter is the environment variable name the
/// target client sets to the project root (e.g. `DEVIN_PROJECT_DIR` for
/// Devin CLI, `CLAUDE_PROJECT_DIR` for Claude Code).
fn resolve_hook_command(project_root: &Path, project_dir_var: &str) -> Result<String> {
    let exe = env::current_exe()?;

    if let Ok(relative) = exe.strip_prefix(project_root) {
        let rel_str = relative.to_string_lossy();
        if rel_str.starts_with("target/release/") || rel_str.starts_with("target/debug/") {
            return Ok(format!(
                "${}/{} hook --source claude",
                project_dir_var, rel_str
            ));
        }
    }

    // Installed binary (cargo install, brew, etc.) — rely on PATH lookup
    Ok("project-lint hook --source claude".to_string())
}

/// Shell snippet that resolves the project-lint binary at runtime without
/// embedding any absolute path. Used by shell-script-based hook installers
/// (windsurf, claude, cursor, generic, git-hooks).
///
/// The snippet checks all known project-root environment variables for a
/// `target/{release,debug}/project-lint` dev build, then falls back to PATH
/// lookup (`project-lint`). This makes the generated hook portable across
/// clones and across AI coding clients without leaking `/Users/<user>/...`.
///
/// Known project-root env vars (per official docs as of 2026-08):
///   - `CLAUDE_PROJECT_DIR`    (Claude Code)
///   - `CURSOR_PROJECT_DIR`    (Cursor; also aliases `CLAUDE_PROJECT_DIR`)
///   - `DEVIN_PROJECT_DIR`     (Devin CLI)
///   - `$PWD`                  (fallback: git hooks run from repo root)
fn shell_bin_resolution_snippet() -> &'static str {
    r#"# Resolve project-lint binary at runtime (no hardcoded paths)
# Checks dev builds relative to known project-root env vars, then PATH
PROJECT_LINT_BIN="project-lint"
for _root in "${CLAUDE_PROJECT_DIR:-}" "${CURSOR_PROJECT_DIR:-}" "${DEVIN_PROJECT_DIR:-}" "${PWD:-}"; do
  if [ -n "$_root" ] && [ -x "$_root/target/release/project-lint" ]; then
    PROJECT_LINT_BIN="$_root/target/release/project-lint"
    break
  fi
  if [ -n "$_root" ] && [ -x "$_root/target/debug/project-lint" ]; then
    PROJECT_LINT_BIN="$_root/target/debug/project-lint"
    break
  fi
done"#
}

async fn install_pi_hook(args: &InstallHookArgs) -> Result<()> {
    // Pi (earendil-works/pi) uses TypeScript extensions, not shell hooks.
    // The extension subscribes to the "tool_call" event and calls project-lint
    // as a subprocess, bridging pi's in-process event model with project-lint's
    // stdin/stdout hook protocol.
    // See: https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md
    let hook_dir = get_hook_dir(&args.dir, ".pi")?;
    let extensions_dir = hook_dir.join("extensions");
    fs::create_dir_all(&extensions_dir)?;

    let extension_content = r#"// project-lint hook extension for pi
// Auto-generated by `project-lint install-hook --agent pi`
//
// This extension bridges pi's in-process tool_call event to project-lint's
// stdin/stdout hook protocol. It sends the event as a Claude Code-compatible
// JSON payload to `project-lint hook --source claude` and applies any
// modified input returned by project-lint back to the tool call.
//
// To install globally instead of project-local, copy this file to:
//   ~/.pi/agent/extensions/project-lint-hook.ts

import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// Resolve the project-lint binary at runtime without embedding any
// absolute path. Checks dev builds relative to known project-root env
// vars and the extension's own location, then falls back to PATH.
function resolveProjectLintBin(): string {
  // Explicit override
  if (process.env.PROJECT_LINT_BIN) return process.env.PROJECT_LINT_BIN;

  // Check known project-root env vars (per official docs as of 2026-08)
  const rootVars = [
    "CLAUDE_PROJECT_DIR",
    "CURSOR_PROJECT_DIR",
    "DEVIN_PROJECT_DIR",
  ];
  for (const v of rootVars) {
    const root = process.env[v];
    if (!root) continue;
    for (const profile of ["release", "debug"]) {
      const candidate = join(root, "target", profile, "project-lint");
      if (existsSync(candidate)) return candidate;
    }
  }

  // Walk up from this extension's directory to find target/{release,debug}/
  // Extension lives at <project_root>/.pi/extensions/project-lint-hook.ts
  let dir = dirname(fileURLToPath(import.meta.url));
  for (let i = 0; i < 10; i++) {
    for (const profile of ["release", "debug"]) {
      const candidate = join(dir, "target", profile, "project-lint");
      if (existsSync(candidate)) return candidate;
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }

  // Fallback: rely on PATH lookup via spawn
  return "project-lint";
}

const PROJECT_LINT_BIN = resolveProjectLintBin();

export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event, _ctx) => {
    // Convert pi's tool_call event to Claude Code hook format
    const hookPayload = JSON.stringify({
      hook_event_name: "PreToolUse",
      tool_name: event.toolName,
      tool_input: event.input,
    });

    return new Promise((resolve) => {
      const child = spawn(PROJECT_LINT_BIN, ["hook", "--source", "claude"], {
        stdio: ["pipe", "pipe", "pipe"],
      });

      let stdout = "";
      let stderr = "";

      child.stdout.on("data", (data: Buffer) => { stdout += data.toString(); });
      child.stderr.on("data", (data: Buffer) => { stderr += data.toString(); });

      child.on("close", (code: number | null) => {
        try {
          if (stdout.trim()) {
            const response = JSON.parse(stdout);

            // Apply modified input to the tool call
            if (response.hookSpecificOutput?.updatedInput) {
              const updated = response.hookSpecificOutput.updatedInput;
              for (const [key, value] of Object.entries(updated)) {
                (event.input as Record<string, unknown>)[key] = value;
              }
            }

            // Block if project-lint denied the action
            if (response.continue === false) {
              resolve({
                block: true,
                reason: response.stopReason || "Blocked by project-lint",
              });
              return;
            }
          }
        } catch {
          // Ignore JSON parse errors — allow the tool call through
        }

        // Exit code 2 means block in Claude Code hook protocol
        if (code === 2) {
          resolve({
            block: true,
            reason: stderr.trim() || "Blocked by project-lint",
          });
          return;
        }

        resolve(undefined);
      });

      child.on("error", () => {
        // If project-lint binary is not found, silently allow the tool call
        resolve(undefined);
      });

      child.stdin.write(hookPayload);
      child.stdin.end();
    });
  });
}
"#;

    let extension_path = extensions_dir.join("project-lint-hook.ts");
    write_hook_file(&extension_path, &extension_content, args.force)?;

    info!("Pi extension installed at {:?}", extension_path);
    info!("Pi auto-discovers extensions from .pi/extensions/ after project trust.");
    info!("Use /reload in pi to hot-reload the extension without restarting.");
    Ok(())
}

async fn install_generic_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, "hooks")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Generic AI agent hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

{bin_resolution}
HOOK_TYPE="generic"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        bin_resolution = shell_bin_resolution_snippet()
    );

    let hook_path = hook_dir.join("project-lint-hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Generic hook installed at {:?}", hook_path);
    Ok(())
}

/// Resolve the protected-branches list to bake into generated git hook
/// scripts. Reads the project config from `project_root` and looks for the
/// `worktree-isolation-enforcer` custom rule's `protected_branches` field.
/// Falls back to the conventional defaults `main master trunk develop` when
/// the config or rule is absent, or when the field is empty.
fn resolve_protected_branches_for_hooks(project_root: &Path) -> String {
    let defaults = "main master trunk develop";
    let config_path = project_root
        .join(".config")
        .join("project-lint")
        .join("config.toml");
    match project_lint_core::config::Config::load_from_file(&config_path) {
        Ok(config) => {
            for rule in &config.rules.custom_rules {
                if rule.name == "worktree-isolation-enforcer" && !rule.protected_branches.is_empty()
                {
                    return rule.protected_branches.join(" ");
                }
            }
            // Also check modular rules
            for modular in &config.modular_rules {
                if let Some(custom_rules) = &modular.rules {
                    for rule in custom_rules {
                        if rule.name == "worktree-isolation-enforcer"
                            && !rule.protected_branches.is_empty()
                        {
                            return rule.protected_branches.join(" ");
                        }
                    }
                }
            }
            defaults.to_string()
        }
        Err(_) => defaults.to_string(),
    }
}

/// Generate the bash worktree-isolation gate snippet for git hooks.
/// `action` is "commits" or "pushes" — used in the user-facing message.
/// `protected_branches` is the space-separated list to bake into the script.
fn worktree_gate_snippet(action: &str, protected_branches: &str) -> String {
    format!(
        r#"# --- Worktree isolation gate -------------------------------------------
# Block {action} to protected branches ({protected_branches}) when not
# inside a linked git worktree. Work on protected branches must happen in a
# worktree (git worktree add) so the main worktree stays clean.
PROTECTED_BRANCHES="{protected_branches}"
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
GIT_DIR_PATH=$(git rev-parse --git-dir 2>/dev/null)
GIT_COMMON_DIR_PATH=$(git rev-parse --git-common-dir 2>/dev/null)

is_protected() {{
  for b in $PROTECTED_BRANCHES; do
    if [ "$CURRENT_BRANCH" = "$b" ]; then return 0; fi
  done
  return 1
}}

# A linked worktree has a git-dir that differs from the git-common-dir.
# In the main worktree both resolve to the same path.
is_linked_worktree() {{
  if [ -z "$GIT_DIR_PATH" ] || [ -z "$GIT_COMMON_DIR_PATH" ]; then
    return 1
  fi
  # Normalize to absolute paths for comparison
  ABS_GIT_DIR=$(cd "$(git rev-parse --show-toplevel)" 2>/dev/null && cd "$GIT_DIR_PATH" && pwd 2>/dev/null || echo "$GIT_DIR_PATH")
  ABS_COMMON_DIR=$(cd "$(git rev-parse --show-toplevel)" 2>/dev/null && cd "$GIT_COMMON_DIR_PATH" && pwd 2>/dev/null || echo "$GIT_COMMON_DIR_PATH")
  [ "$ABS_GIT_DIR" != "$ABS_COMMON_DIR" ]
}}

if is_protected && ! is_linked_worktree; then
  echo "🚫 Worktree isolation: {action} to '$CURRENT_BRANCH' are blocked outside a linked worktree."
  echo ""
  echo "Create a worktree before working on $CURRENT_BRANCH:"
  echo "  git worktree add ../$CURRENT_BRANCH-work -b work/$CURRENT_BRANCH"
  echo "Then {action_verb} from inside that worktree."
  exit 1
fi
# --- End worktree isolation gate ---------------------------------------"#,
        action = action,
        action_verb = if action == "commits" {
            "commit"
        } else {
            "push"
        },
        protected_branches = protected_branches,
    )
}

async fn install_git_hooks(args: &InstallHookArgs) -> Result<()> {
    let git_dir = get_hook_dir(&args.dir, ".git")?;
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Derive the project root from the .git directory parent (the repo root
    // in a standard layout). This ensures we read the target repo's config,
    // not the installer's cwd, when baking protected branches into hooks.
    let project_root = git_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Read protected branches from config (falls back to defaults).
    let protected_branches = resolve_protected_branches_for_hooks(&project_root);

    // Install pre-commit hook
    let pre_commit_content = format!(
        r#"#!/bin/bash
# Pre-commit hook for project-lint
# Runs project-lint before committing changes

{bin_resolution}

{worktree_gate}

# Run project-lint on staged files
echo "Running project-lint pre-commit checks..."

# Get list of staged files
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM)

if [ -z "$STAGED_FILES" ]; then
    echo "No staged files to check"
    exit 0
fi

# Run project-lint
"$PROJECT_LINT_BIN" lint --fix --dry-run
LINT_EXIT_CODE=$?

if [ $LINT_EXIT_CODE -ne 0 ]; then
    echo "\n❌ project-lint found issues. Please fix them before committing."
    echo "Run 'project-lint lint --fix' to auto-fix issues."
    exit 1
fi

echo "✅ project-lint checks passed"
exit 0
"#,
        bin_resolution = shell_bin_resolution_snippet(),
        worktree_gate = worktree_gate_snippet("commits", &protected_branches),
    );

    let pre_commit_path = hooks_dir.join("pre-commit");
    write_hook_file(&pre_commit_path, &pre_commit_content, args.force)?;
    make_executable(&pre_commit_path)?;

    // Install pre-push hook
    let pre_push_content = format!(
        r#"#!/bin/bash
# Pre-push hook for project-lint
# Runs comprehensive project-lint checks before pushing

{bin_resolution}

{worktree_gate}

# Run full project-lint check
echo "Running project-lint pre-push checks..."

"$PROJECT_LINT_BIN" lint --fix --dry-run
LINT_EXIT_CODE=$?

if [ $LINT_EXIT_CODE -ne 0 ]; then
    echo "\n❌ project-lint found issues. Please fix them before pushing."
    echo "Run 'project-lint lint --fix' to auto-fix issues."
    exit 1
fi

echo "✅ project-lint checks passed"
exit 0
"#,
        bin_resolution = shell_bin_resolution_snippet(),
        worktree_gate = worktree_gate_snippet("pushes", &protected_branches),
    );

    let pre_push_path = hooks_dir.join("pre-push");
    write_hook_file(&pre_push_path, &pre_push_content, args.force)?;
    make_executable(&pre_push_path)?;

    info!("Git hooks installed at {:?}", hooks_dir);
    Ok(())
}

async fn install_github_workflow(args: &InstallHookArgs) -> Result<()> {
    let workflow_dir = get_hook_dir(&args.dir, ".github/workflows")?;
    fs::create_dir_all(&workflow_dir)?;

    let _project_lint_bin = env::current_exe()?.to_string_lossy().to_string();

    // Create GitHub Actions workflow
    let workflow_content = r#"name: Project-Lint

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

jobs:
  project-lint:
    runs-on: ubuntu-latest

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        components: rustfmt, clippy

    - name: Cache cargo registry
      uses: actions/cache@v3
      with:
        path: ~/.cargo/registry
        key: ${{ runner.os }}-cargo-registry-${{ hash('**/Cargo.lock') }}

    - name: Build project-lint
      run: |
        cargo build --release --bin project-lint

    - name: Run project-lint
      run: |
        ./target/release/project-lint lint --fix --dry-run

    - name: Run project-lint with stats
      run: |
        ./target/release/project-lint logs --stats

    - name: Upload lint results
      if: failure()
      uses: actions/upload-artifact@v3
      with:
        name: lint-results
        path: |
          project-lint.log
          .local/share/project-lint/logs/

  security-scan:
    runs-on: ubuntu-latest

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Build project-lint
      run: cargo build --release --bin project-lint

    - name: Run security scan
      run: |
        ./target/release/project-lint lint --fix --dry-run

    - name: Check for security issues
      run: |
        if ./target/release/project-lint logs --stats | grep -q "error"; then
          echo "Security issues found"
          exit 1
        fi
"#
    .to_string();

    let workflow_path = workflow_dir.join("project-lint.yml");
    write_hook_file(&workflow_path, &workflow_content, args.force)?;

    // Create PR workflow
    let pr_workflow_content = r#"name: Project-Lint PR Check

on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  lint-pr:
    runs-on: ubuntu-latest

    steps:
    - name: Checkout code
      uses: actions/checkout@v4
      with:
        fetch-depth: 0

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Build project-lint
      run: cargo build --release --bin project-lint

    - name: Get changed files
      id: changed-files
      run: |
        echo "changed_files=$(git diff --name-only origin/${{ github.base_ref }}..HEAD | tr '\n' ' ')" >> $GITHUB_OUTPUT

    - name: Run project-lint on changed files
      run: |
        if [ -n "${{ steps.changed-files.outputs.changed_files }}" ]; then
          ./target/release/project-lint lint --fix --dry-run
        else
          echo "No files changed"
        fi

    - name: Comment on PR
      if: failure()
      uses: actions/github-script@v6
      with:
        script: |
          github.rest.issues.createComment({
            issue_number: context.issue.number,
            owner: context.repo.owner,
            repo: context.repo.repo,
            body: '🚫 project-lint found issues in this PR. Please run `project-lint lint --fix` to fix them.'
          })
"#.to_string();

    let pr_workflow_path = workflow_dir.join("project-lint-pr.yml");
    write_hook_file(&pr_workflow_path, &pr_workflow_content, args.force)?;

    info!("GitHub workflows installed at {:?}", workflow_dir);
    Ok(())
}

async fn install_gitlab_workflow(args: &InstallHookArgs) -> Result<()> {
    let workflow_dir = get_hook_dir(&args.dir, ".gitlab-ci.yml")?;

    let _project_lint_bin = env::current_exe()?.to_string_lossy().to_string();

    // Create GitLab CI configuration
    let gitlab_ci_content = r#"# GitLab CI configuration for project-lint
stages:
  - lint
  - security
  - deploy

variables:
  CARGO_HOME: "$CI_PROJECT_DIR/.cargo"
  RUST_BACKTRACE: "1"

cache:
  key: "$CI_COMMIT_REF_SLUG"
  paths:
    - .cargo/
    - target/

# Lint stage
lint:
  stage: lint
  image: rust:latest
  before_script:
    - apt-get update -y && apt-get install -y pkg-config
    - rustup component add rustfmt clippy
  script:
    - cargo build --release --bin project-lint
    - ./target/release/project-lint lint --fix --dry-run
    - ./target/release/project-lint logs --stats
  artifacts:
    when: always
    reports:
      junit: lint-results.xml
    paths:
      - project-lint.log
      - .local/share/project-lint/logs/
    expire_in: 1 week
  allow_failure: false

# Security scan
security-scan:
  stage: security
  image: rust:latest
  dependencies:
    - lint
  script:
    - cargo build --release --bin project-lint
    - ./target/release/project-lint lint --fix --dry-run
    - |
      if ./target/release/project-lint logs --stats | grep -q "error"; then
        echo "Security issues found"
        exit 1
      fi
  artifacts:
    when: always
    reports:
      security: security-report.json
    paths:
      - security-report.json
    expire_in: 1 week
  allow_failure: false

# PR-specific job
lint-merge-request:
  stage: lint
  image: rust:latest
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  before_script:
    - apt-get update -y && apt-get install -y pkg-config git
    - rustup component add rustfmt clippy
  script:
    - cargo build --release --bin project-lint
    - |
      # Get changed files in MR
      CHANGED_FILES=$(git diff --name-only $CI_MERGE_REQUEST_TARGET_BRANCH_NAME..HEAD)
      if [ -n "$CHANGED_FILES" ]; then
        echo "Changed files: $CHANGED_FILES"
        ./target/release/project-lint lint --fix --dry-run
      else
        echo "No files changed"
      fi
    - ./target/release/project-lint logs --stats
  artifacts:
    when: always
    paths:
      - mr-lint-results.log
      - .local/share/project-lint/logs/
    expire_in: 1 week
  allow_failure: false

# Scheduled security scan
scheduled-security-scan:
  stage: security
  image: rust:latest
  rules:
    - if: $CI_PIPELINE_SOURCE == "schedule"
  script:
    - cargo build --release --bin project-lint
    - ./target/release/project-lint lint --fix --dry-run
    - |
      # Generate security report
      ./target/release/project-lint logs --stats > security-scan-report.txt
      echo "Security scan completed on $(date)" >> security-scan-report.txt
  artifacts:
    paths:
      - security-scan-report.txt
    expire_in: 1 month
  allow_failure: true

# Deploy stage (example)
deploy:
  stage: deploy
  image: alpine:latest
  dependencies:
    - lint
    - security-scan
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
  script:
    - echo "Deploying to production..."
    - echo "All lint and security checks passed"
  environment:
    name: production
    url: https://example.com
  when: manual
"#
    .to_string();

    write_hook_file(&workflow_dir, &gitlab_ci_content, args.force)?;

    // Create GitLab MR template
    let mr_template_dir = get_hook_dir(&args.dir, ".gitlab/merge_request_templates")?;
    fs::create_dir_all(&mr_template_dir)?;

    let mr_template = r#"## Project-Lint Results

### Lint Status
- [ ] All lint checks passed
- [ ] No security issues found
- [ ] Code follows project standards

### Checklist
- [ ] I have run `project-lint lint --fix`
- [ ] I have reviewed the security scan results
- [ ] I have tested my changes
- [ ] Documentation is updated if needed

### Additional Notes

<!-- Add any additional context about your changes here -->
"#;

    let mr_template_path = mr_template_dir.join("project-lint.md");
    write_hook_file(&mr_template_path, mr_template, args.force)?;

    info!("GitLab CI configuration installed at {:?}", workflow_dir);
    Ok(())
}

fn get_hook_dir(custom_dir: &Option<String>, default_subdir: &str) -> Result<PathBuf> {
    if let Some(dir) = custom_dir {
        Ok(PathBuf::from(dir))
    } else {
        let cwd = env::current_dir()?;
        Ok(cwd.join(default_subdir))
    }
}

fn write_hook_file(path: &PathBuf, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        warn!(
            "Hook file already exists at {:?}. Use --force to overwrite.",
            path
        );
        return Ok(());
    }

    fs::write(path, content)?;
    Ok(())
}

fn make_executable(path: &PathBuf) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_lint_core::utils::Result;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;
    use tokio::fs as async_fs;

    #[tokio::test]
    async fn test_install_windsurf_hook() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".windsurf");

        let args = InstallHookArgs {
            agent: "windsurf".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // Check hook script was created
        let hook_script = hook_dir.join("hook.sh");
        assert!(hook_script.exists());

        // Check script is executable
        let metadata = fs::metadata(&hook_script)?;
        #[cfg(unix)]
        assert!(metadata.permissions().mode() & 0o111 != 0);

        // Check config file was created
        let config_file = hook_dir.join("config.toml");
        assert!(config_file.exists());

        // Verify script content
        let content = fs::read_to_string(&hook_script)?;
        assert!(content.contains("hook --source"));
        assert!(content.contains("HOOK_TYPE=\"windsurf\""));
        // No hardcoded absolute path — must use runtime resolution
        assert!(
            !content.contains("/Users/"),
            "windsurf hook must not embed an absolute home path"
        );
        assert!(
            content.contains("CLAUDE_PROJECT_DIR") || content.contains("PROJECT_LINT_BIN"),
            "windsurf hook must use runtime bin resolution"
        );
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".claude");

        let args = InstallHookArgs {
            agent: "claude".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // Check hook script was created
        let hook_script = hook_dir.join("hook.sh");
        assert!(hook_script.exists());

        // Verify script content
        let content = fs::read_to_string(&hook_script)?;
        assert!(content.contains("hook --source"));
        assert!(content.contains("HOOK_TYPE=\"claude\""));
        assert!(
            !content.contains("/Users/"),
            "claude hook must not embed an absolute home path"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_install_claude_hook_creates_settings_json() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".claude");

        let args = InstallHookArgs {
            agent: "claude".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // settings.json must be created and register PreToolUse, PostToolUse,
        // Stop, and SubagentStop.
        let settings_path = hook_dir.join("settings.json");
        assert!(settings_path.exists(), "settings.json must be created");
        let content = fs::read_to_string(&settings_path)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;

        let pre = &parsed["hooks"]["PreToolUse"];
        assert!(pre.is_array(), "PreToolUse hooks must be an array");
        let matcher = pre[0]["matcher"].as_str().unwrap_or("");
        assert!(
            matcher.contains("Edit") && matcher.contains("Task"),
            "PreToolUse matcher must cover Edit and Task, got: {matcher}"
        );
        let cmd = pre[0]["hooks"][0]["command"].as_str().unwrap_or("");
        assert!(
            cmd.contains(".claude/hook.sh"),
            "command must point at hook.sh"
        );

        let post = &parsed["hooks"]["PostToolUse"];
        assert!(post.is_array(), "PostToolUse hooks must be an array");
        let post_matcher = post[0]["matcher"].as_str().unwrap_or("");
        assert!(
            post_matcher.contains("Edit") && post_matcher.contains("Write"),
            "PostToolUse matcher must cover Edit and Write, got: {post_matcher}"
        );

        let stop = &parsed["hooks"]["Stop"];
        assert!(stop.is_array(), "Stop hooks must be an array");
        let stop_cmd = stop[0]["hooks"][0]["command"].as_str().unwrap_or("");
        assert!(stop_cmd.contains(".claude/hook.sh"));

        let subagent_stop = &parsed["hooks"]["SubagentStop"];
        assert!(
            subagent_stop.is_array(),
            "SubagentStop hooks must be an array"
        );
        let ss_cmd = subagent_stop[0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or("");
        assert!(ss_cmd.contains(".claude/hook.sh"));

        Ok(())
    }

    #[tokio::test]
    async fn test_install_claude_hook_merges_existing_settings() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".claude");
        fs::create_dir_all(&hook_dir)?;

        // Pre-existing settings with a user permission that must survive.
        let existing = r#"{
          "permissions": { "allow": ["Bash(git:*)"] },
          "hooks": {
            "PreToolUse": [
              { "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo hi" }] }
            ]
          }
        }"#;
        fs::write(hook_dir.join("settings.json"), existing)?;

        let args = InstallHookArgs {
            agent: "claude".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };
        run(args).await?;

        let content = fs::read_to_string(hook_dir.join("settings.json"))?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;

        // User permission preserved.
        assert_eq!(
            parsed["permissions"]["allow"][0],
            serde_json::json!("Bash(git:*)")
        );
        // User's existing PreToolUse entry preserved.
        let pre = parsed["hooks"]["PreToolUse"].as_array().unwrap();
        assert!(
            pre.iter().any(|e| e["matcher"].as_str() == Some("Bash")),
            "existing Bash matcher must be preserved"
        );
        // Our entry added.
        assert!(
            pre.iter()
                .any(|e| e["matcher"].as_str().unwrap_or("").contains("Edit")),
            "project-lint Edit matcher must be added"
        );
        // Stop added.
        assert!(parsed["hooks"]["Stop"].is_array());

        Ok(())
    }

    #[tokio::test]
    async fn test_install_all_installs_git_and_claude_hooks() -> Result<()> {
        let temp_dir = TempDir::new()?;
        // `all` forwards the same --dir to both installers. With a single
        // --dir, git hooks land at <dir>/hooks/ and claude hooks at <dir>/,
        // which is enough to verify both were installed without mutating the
        // process-global cwd (unsafe under parallel test threads).
        let target = temp_dir.path().join("hooks-root");
        fs::create_dir_all(&target)?;

        let args = InstallHookArgs {
            agent: "all".to_string(),
            dir: Some(target.to_string_lossy().to_string()),
            force: true,
        };
        run(args).await?;

        // Git hooks installed.
        let pre_commit = target.join("hooks").join("pre-commit");
        assert!(pre_commit.exists(), "git pre-commit must be installed");
        let pc_content = fs::read_to_string(&pre_commit)?;
        assert!(
            pc_content.contains("Worktree isolation"),
            "pre-commit must contain the worktree isolation gate"
        );

        // Pre-push hook also has the worktree gate.
        let pre_push = target.join("hooks").join("pre-push");
        assert!(pre_push.exists(), "git pre-push must be installed");
        let pp_content = fs::read_to_string(&pre_push)?;
        assert!(
            pp_content.contains("Worktree isolation"),
            "pre-push must contain the worktree isolation gate"
        );
        assert!(
            pp_content.contains("pushes"),
            "pre-push gate message must mention pushes"
        );

        // Claude hooks installed.
        let claude_hook = target.join("hook.sh");
        assert!(claude_hook.exists(), ".claude/hook.sh must be installed");
        let settings = target.join("settings.json");
        assert!(settings.exists(), ".claude/settings.json must be installed");

        Ok(())
    }

    #[tokio::test]
    async fn test_install_cursor_hook() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".cursor");

        let args = InstallHookArgs {
            agent: "cursor".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // Check hook script was created
        let hook_script = hook_dir.join("hook.sh");
        assert!(hook_script.exists());

        // Verify script content
        let content = fs::read_to_string(&hook_script)?;
        assert!(content.contains("hook --source"));
        assert!(content.contains("HOOK_TYPE=\"cursor\""));
        assert!(
            !content.contains("/Users/"),
            "cursor hook must not embed an absolute home path"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_install_generic_hook() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join("hooks");

        let args = InstallHookArgs {
            agent: "generic".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // Check hook script was created
        let hook_script = hook_dir.join("project-lint-hook.sh");
        assert!(hook_script.exists());

        // Verify script content
        let content = fs::read_to_string(&hook_script)?;
        assert!(content.contains("hook --source"));
        assert!(content.contains("HOOK_TYPE=\"generic\""));
        assert!(
            !content.contains("/Users/"),
            "generic hook must not embed an absolute home path"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_install_hook_force_overwrite() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".windsurf");
        fs::create_dir_all(&hook_dir)?;

        // Create existing hook
        let existing_hook = hook_dir.join("hook.sh");
        fs::write(&existing_hook, "existing content")?;

        let args = InstallHookArgs {
            agent: "windsurf".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        // Should not overwrite without force
        run(args).await?;
        let content = fs::read_to_string(&existing_hook)?;
        assert_eq!(content, "existing content");

        // Now with force
        let args = InstallHookArgs {
            agent: "windsurf".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: true,
        };

        run(args).await?;
        let content = fs::read_to_string(&existing_hook)?;
        assert!(content.contains("hook --source"));
        assert!(content.contains("HOOK_TYPE=\"windsurf\""));

        Ok(())
    }

    #[tokio::test]
    async fn test_install_hook_unsupported_agent() -> Result<()> {
        let temp_dir = TempDir::new()?;

        let args = InstallHookArgs {
            agent: "unsupported".to_string(),
            dir: Some(temp_dir.path().to_string_lossy().to_string()),
            force: false,
        };

        let result = run(args).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_install_devin_hook() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".devin");

        let args = InstallHookArgs {
            agent: "devin".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // Check hooks.v1.json was created
        let hooks_file = hook_dir.join("hooks.v1.json");
        assert!(hooks_file.exists());

        // Verify content is valid JSON with the expected structure
        let content = fs::read_to_string(&hooks_file)?;
        let parsed: serde_json::Value = serde_json::from_str(&content)?;
        assert!(parsed["PreToolUse"].is_array());
        assert!(parsed["PostToolUse"].is_array());

        // Verify the hook command references project-lint
        let pre_tool_hooks = parsed["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .expect("command should be a string");
        assert!(pre_tool_hooks.contains("hook --source claude"));

        // Verify no absolute path leak — command must not start with /
        // (should be either bare "project-lint" or "$DEVIN_PROJECT_DIR/...")
        assert!(
            !pre_tool_hooks.starts_with('/'),
            "hook command must not contain an absolute path, got: {}",
            pre_tool_hooks
        );

        Ok(())
    }

    #[test]
    fn test_resolve_hook_command_dev_build() {
        // CARGO_MANIFEST_DIR is set by cargo during test builds and points
        // to the directory containing the package's Cargo.toml — the project
        // root. The test binary lives under <project_root>/target/debug/deps/,
        // so resolve_hook_command should emit a $DEVIN_PROJECT_DIR path.
        let manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR should be set during cargo test");
        let project_root = Path::new(&manifest_dir);

        let cmd = resolve_hook_command(project_root, "DEVIN_PROJECT_DIR")
            .expect("resolve_hook_command should succeed");
        assert!(
            cmd.starts_with("$DEVIN_PROJECT_DIR/target/"),
            "dev build should use $DEVIN_PROJECT_DIR, got: {}",
            cmd
        );
        assert!(cmd.ends_with("hook --source claude"));
    }

    #[test]
    fn test_resolve_hook_command_path_install() {
        // When project_root is unrelated to current_exe (e.g. a tempdir),
        // resolve_hook_command should fall back to bare PATH-based lookup.
        let temp_dir = TempDir::new().expect("TempDir::new should work");
        let cmd = resolve_hook_command(temp_dir.path(), "DEVIN_PROJECT_DIR")
            .expect("resolve_hook_command should succeed");
        assert_eq!(
            cmd, "project-lint hook --source claude",
            "non-dev-build should fall back to bare PATH lookup"
        );
    }

    #[tokio::test]
    async fn test_install_pi_hook() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let hook_dir = temp_dir.path().join(".pi");

        let args = InstallHookArgs {
            agent: "pi".to_string(),
            dir: Some(hook_dir.to_string_lossy().to_string()),
            force: false,
        };

        run(args).await?;

        // Check TypeScript extension was created
        let extension_file = hook_dir.join("extensions").join("project-lint-hook.ts");
        assert!(extension_file.exists());

        // Verify content is a TypeScript extension
        let content = fs::read_to_string(&extension_file)?;
        assert!(content.contains("ExtensionAPI"));
        assert!(content.contains("pi.on(\"tool_call\""));
        assert!(content.contains("hook --source claude"));
        assert!(content.contains("spawn"));
        assert!(content.contains("hookSpecificOutput"));
        assert!(content.contains("updatedInput"));
        // No hardcoded absolute path — must use runtime resolution
        assert!(
            !content.contains("/Users/"),
            "pi extension must not embed an absolute home path"
        );
        assert!(
            content.contains("resolveProjectLintBin"),
            "pi extension must use runtime bin resolution"
        );

        Ok(())
    }

    #[test]
    fn test_shell_bin_resolution_snippet_no_absolute_path() {
        let snippet = shell_bin_resolution_snippet();
        // Must not contain any hardcoded absolute path
        assert!(
            !snippet.contains("/Users/"),
            "shell resolution snippet must not contain /Users/"
        );
        assert!(
            !snippet.contains("/home/"),
            "shell resolution snippet must not contain /home/"
        );
        // Must check all known project-root env vars
        assert!(snippet.contains("CLAUDE_PROJECT_DIR"));
        assert!(snippet.contains("CURSOR_PROJECT_DIR"));
        assert!(snippet.contains("DEVIN_PROJECT_DIR"));
        // Must fall back to PATH-based lookup
        assert!(
            snippet.contains("\"project-lint\""),
            "shell resolution snippet must default to PATH-based project-lint"
        );
    }
}
