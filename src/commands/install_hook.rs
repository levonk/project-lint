use clap::Args;
use project_lint_core::utils::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

#[derive(Args)]
pub struct InstallHookArgs {
    /// Target agent (windsurf, claude, cursor, devin, pi, generic, git-hooks, github, gitlab)
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

PROJECT_LINT_BIN="{}"
HOOK_TYPE="windsurf"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
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

PROJECT_LINT_BIN="{}"
HOOK_TYPE="claude"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Claude Code hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_cursor_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".cursor")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Cursor hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

PROJECT_LINT_BIN="{}"
HOOK_TYPE="cursor"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
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

/// Resolve the hook command string for a generated hook config.
///
/// If the currently running binary is inside `<project_root>/target/{release,debug}/`,
/// emits a `$<project_dir_var>/target/...` path so the hook resolves the binary
/// relative to the project root on any clone (no absolute home path leak).
/// Otherwise, emits a bare `project-lint` command that relies on `$PATH` lookup
/// (cargo install, brew, devbox, etc.).
///
/// The `project_dir_var` parameter is the environment variable name the target
/// client sets to the project root (e.g. `DEVIN_PROJECT_DIR` for Devin CLI,
/// `CLAUDE_PROJECT_DIR` for Claude Code).
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

async fn install_pi_hook(args: &InstallHookArgs) -> Result<()> {
    // Pi (earendil-works/pi) uses TypeScript extensions, not shell hooks.
    // The extension subscribes to the "tool_call" event and calls project-lint
    // as a subprocess, bridging pi's in-process event model with project-lint's
    // stdin/stdout hook protocol.
    // See: https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md
    let hook_dir = get_hook_dir(&args.dir, ".pi")?;
    let extensions_dir = hook_dir.join("extensions");
    fs::create_dir_all(&extensions_dir)?;

    let project_lint_bin = env::current_exe()?.to_string_lossy().to_string();

    let extension_content = format!(
        r#"// project-lint hook extension for pi
// Auto-generated by `project-lint install-hook --agent pi`
//
// This extension bridges pi's in-process tool_call event to project-lint's
// stdin/stdout hook protocol. It sends the event as a Claude Code-compatible
// JSON payload to `project-lint hook --source claude` and applies any
// modified input returned by project-lint back to the tool call.
//
// To install globally instead of project-local, copy this file to:
//   ~/.pi/agent/extensions/project-lint-hook.ts

import type {{ ExtensionAPI }} from "@earendil-works/pi-coding-agent";
import {{ spawn }} from "node:child_process";

const PROJECT_LINT_BIN = process.env.PROJECT_LINT_BIN || "{bin}";

export default function (pi: ExtensionAPI) {{
  pi.on("tool_call", async (event, _ctx) => {{
    // Convert pi's tool_call event to Claude Code hook format
    const hookPayload = JSON.stringify({{
      hook_event_name: "PreToolUse",
      tool_name: event.toolName,
      tool_input: event.input,
    }});

    return new Promise((resolve) => {{
      const child = spawn(PROJECT_LINT_BIN, ["hook", "--source", "claude"], {{
        stdio: ["pipe", "pipe", "pipe"],
      }});

      let stdout = "";
      let stderr = "";

      child.stdout.on("data", (data: Buffer) => {{ stdout += data.toString(); }});
      child.stderr.on("data", (data: Buffer) => {{ stderr += data.toString(); }});

      child.on("close", (code: number | null) => {{
        try {{
          if (stdout.trim()) {{
            const response = JSON.parse(stdout);

            // Apply modified input to the tool call
            if (response.hookSpecificOutput?.updatedInput) {{
              const updated = response.hookSpecificOutput.updatedInput;
              for (const [key, value] of Object.entries(updated)) {{
                (event.input as Record<string, unknown>)[key] = value;
              }}
            }}

            // Block if project-lint denied the action
            if (response.continue === false) {{
              resolve({{
                block: true,
                reason: response.stopReason || "Blocked by project-lint",
              }});
              return;
            }}
          }}
        }} catch {{
          // Ignore JSON parse errors — allow the tool call through
        }}

        // Exit code 2 means block in Claude Code hook protocol
        if (code === 2) {{
          resolve({{
            block: true,
            reason: stderr.trim() || "Blocked by project-lint",
          }});
          return;
        }}

        resolve(undefined);
      }});

      child.on("error", () => {{
        // If project-lint binary is not found, silently allow the tool call
        resolve(undefined);
      }});

      child.stdin.write(hookPayload);
      child.stdin.end();
    }});
  }});
}}
"#,
        bin = project_lint_bin
    );

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

PROJECT_LINT_BIN="{}"
HOOK_TYPE="generic"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
    );

    let hook_path = hook_dir.join("project-lint-hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Generic hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_git_hooks(args: &InstallHookArgs) -> Result<()> {
    let git_dir = get_hook_dir(&args.dir, ".git")?;
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let project_lint_bin = env::current_exe()?.to_string_lossy().to_string();

    // Install pre-commit hook
    let pre_commit_content = format!(
        r#"#!/bin/bash
# Pre-commit hook for project-lint
# Runs project-lint before committing changes

PROJECT_LINT_BIN="{}"

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
        project_lint_bin
    );

    let pre_commit_path = hooks_dir.join("pre-commit");
    write_hook_file(&pre_commit_path, &pre_commit_content, args.force)?;
    make_executable(&pre_commit_path)?;

    // Install pre-push hook
    let pre_push_content = format!(
        r#"#!/bin/bash
# Pre-push hook for project-lint
# Runs comprehensive project-lint checks before pushing

PROJECT_LINT_BIN="{}"

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
        project_lint_bin
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

        Ok(())
    }
}
