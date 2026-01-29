use crate::utils::Result;
use clap::Args;
use std::path::PathBuf;
use std::fs;
use std::env;
use tracing::{info, warn, error};

#[derive(Args)]
pub struct InstallHookArgs {
    /// Target agent (windsurf, claude, cursor, generic, git-hooks, github, gitlab)
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

async fn install_generic_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, "hooks")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r"""#!/bin/bash
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
""",
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

    let project_lint_bin = env::current_exe()?.display();

    // Install pre-commit hook
    let pre_commit_content = format!(
        r"""#!/bin/bash
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
""",
        project_lint_bin
    );

    let pre_commit_path = hooks_dir.join("pre-commit");
    write_hook_file(&pre_commit_path, &pre_commit_content, args.force)?;
    make_executable(&pre_commit_path)?;

    // Install pre-push hook
    let pre_push_content = format!(
        r"""#!/bin/bash
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
""",
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

    let project_lint_bin = env::current_exe()?.display();

    // Create GitHub Actions workflow
    let workflow_content = format!(
        r"""name: Project-Lint

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
        key: ${{{{ runner.os }}}-cargo-registry-${{ hash('**/Cargo.lock') }}}

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
""",
        project_lint_bin
    );

    let workflow_path = workflow_dir.join("project-lint.yml");
    write_hook_file(&workflow_path, &workflow_content, args.force)?;

    // Create PR workflow
    let pr_workflow_content = format!(
        r"""name: Project-Lint PR Check

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
""",
        project_lint_bin
    );

    let pr_workflow_path = workflow_dir.join("project-lint-pr.yml");
    write_hook_file(&pr_workflow_path, &pr_workflow_content, args.force)?;

    info!("GitHub workflows installed at {:?}", workflow_dir);
    Ok(())
}

async fn install_gitlab_workflow(args: &InstallHookArgs) -> Result<()> {
    let workflow_dir = get_hook_dir(&args.dir, ".gitlab-ci.yml")?;

    let project_lint_bin = env::current_exe()?.display();

    // Create GitLab CI configuration
    let gitlab_ci_content = format!(
        r"""# GitLab CI configuration for project-lint
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
""",
        project_lint_bin
    );

    write_hook_file(&workflow_dir, &gitlab_ci_content, args.force)?;

    // Create GitLab MR template
    let mr_template_dir = get_hook_dir(&args.dir, ".gitlab/merge_request_templates")?;
    fs::create_dir_all(&mr_template_dir)?;

    let mr_template = r"""## Project-Lint Results

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
""";

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
        warn!("Hook file already exists at {:?}. Use --force to overwrite.", path);
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
    mod install_hook_tests;
}
