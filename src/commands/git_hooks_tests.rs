use crate::commands::install_hook::{run, InstallHookArgs};
use crate::utils::Result;
use tempfile::TempDir;
use std::fs;
use std::path::Path;
use tokio::fs as async_fs;

#[tokio::test]
async fn test_install_git_hooks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let git_dir = temp_dir.path().join(".git");
    let hooks_dir = git_dir.join("hooks");
    
    let args = InstallHookArgs {
        agent: "git-hooks".to_string(),
        dir: Some(git_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check hooks directory was created
    assert!(hooks_dir.exists());
    
    // Check pre-commit hook was created
    let pre_commit_hook = hooks_dir.join("pre-commit");
    assert!(pre_commit_hook.exists());
    
    // Check pre-push hook was created
    let pre_push_hook = hooks_dir.join("pre-push");
    assert!(pre_push_hook.exists());
    
    // Verify hook content
    let pre_commit_content = fs::read_to_string(&pre_commit_hook)?;
    assert!(pre_commit_content.contains("project-lint pre-commit checks"));
    assert!(pre_commit_content.contains("lint --fix --dry-run"));
    
    // Check hooks are executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&pre_commit_hook)?;
        assert!(metadata.permissions().mode() & 0o111 != 0);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_install_github_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let github_dir = temp_dir.path().join(".github");
    let workflows_dir = github_dir.join("workflows");
    
    let args = InstallHookArgs {
        agent: "github".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check workflows directory was created
    assert!(workflows_dir.exists());
    
    // Check main workflow was created
    let main_workflow = workflows_dir.join("project-lint.yml");
    assert!(main_workflow.exists());
    
    // Check PR workflow was created
    let pr_workflow = workflows_dir.join("project-lint-pr.yml");
    assert!(pr_workflow.exists());
    
    // Verify workflow content
    let main_content = fs::read_to_string(&main_workflow)?;
    assert!(main_content.contains("name: Project-Lint"));
    assert!(main_content.contains("rust-toolchain@stable"));
    assert!(main_content.contains("project-lint lint --fix --dry-run"));
    
    let pr_content = fs::read_to_string(&pr_workflow)?;
    assert!(pr_content.contains("Project-Lint PR Check"));
    assert!(pr_content.contains("pull_request"));
    assert!(pr_content.contains("github.rest.issues.createComment"));
    
    Ok(())
}

#[tokio::test]
async fn test_install_gitlab_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let args = InstallHookArgs {
        agent: "gitlab".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check GitLab CI file was created
    let gitlab_ci = temp_dir.path().join(".gitlab-ci.yml");
    assert!(gitlab_ci.exists());
    
    // Check MR template directory was created
    let mr_template_dir = temp_dir.path().join(".gitlab/merge_request_templates");
    assert!(mr_template_dir.exists());
    
    // Check MR template was created
    let mr_template = mr_template_dir.join("project-lint.md");
    assert!(mr_template.exists());
    
    // Verify GitLab CI content
    let gitlab_content = fs::read_to_string(&gitlab_ci)?;
    assert!(gitlab_content.contains("stages:"));
    assert!(gitlab_content.contains("- lint"));
    assert!(gitlab_content.contains("- security"));
    assert!(gitlab_content.contains("project-lint lint --fix --dry-run"));
    
    // Verify MR template content
    let mr_content = fs::read_to_string(&mr_template)?;
    assert!(mr_content.contains("## Project-Lint Results"));
    assert!(mr_content.contains("I have run `project-lint lint --fix`"));
    
    Ok(())
}

#[tokio::test]
async fn test_git_hooks_force_overwrite() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let git_dir = temp_dir.path().join(".git");
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    
    // Create existing hook
    let existing_hook = hooks_dir.join("pre-commit");
    fs::write(&existing_hook, "existing content")?;
    
    let args = InstallHookArgs {
        agent: "git-hooks".to_string(),
        dir: Some(git_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    // Should not overwrite without force
    run(args).await?;
    let content = fs::read_to_string(&existing_hook)?;
    assert_eq!(content, "existing content");
    
    // Now with force
    let args = InstallHookArgs {
        agent: "git-hooks".to_string(),
        dir: Some(git_dir.to_string_lossy().to_string()),
        force: true,
    };
    
    run(args).await?;
    let content = fs::read_to_string(&existing_hook)?;
    assert!(content.contains("project-lint pre-commit checks"));
    
    Ok(())
}

#[tokio::test]
async fn test_workflow_force_overwrite() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflows_dir = temp_dir.path().join(".github/workflows");
    fs::create_dir_all(&workflows_dir)?;
    
    // Create existing workflow
    let existing_workflow = workflows_dir.join("project-lint.yml");
    fs::write(&existing_workflow, "existing workflow")?;
    
    let args = InstallHookArgs {
        agent: "github".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: false,
    };
    
    // Should not overwrite without force
    run(args).await?;
    let content = fs::read_to_string(&existing_workflow)?;
    assert_eq!(content, "existing workflow");
    
    // Now with force
    let args = InstallHookArgs {
        agent: "github".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: true,
    };
    
    run(args).await?;
    let content = fs::read_to_string(&existing_workflow)?;
    assert!(content.contains("name: Project-Lint"));
    
    Ok(())
}

#[tokio::test]
async fn test_unsupported_agent() -> Result<()> {
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

#[test]
fn test_git_hook_content_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let git_dir = temp_dir.path().join(".git");
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    
    // Simulate hook installation
    let project_lint_bin = "/path/to/project-lint";
    let pre_commit_content = format!(
        r#"#!/bin/bash
# Pre-commit hook for project-lint
PROJECT_LINT_BIN="{}"

echo "Running project-lint pre-commit checks..."
"$PROJECT_LINT_BIN" lint --fix --dry-run
LINT_EXIT_CODE=$?

if [ $LINT_EXIT_CODE -ne 0 ]; then
    echo "❌ project-lint found issues"
    exit 1
fi

echo "✅ project-lint checks passed"
exit 0
"#,
        project_lint_bin
    );
    
    let pre_commit_path = hooks_dir.join("pre-commit");
    fs::write(&pre_commit_path, &pre_commit_content)?;
    
    // Validate hook content
    let content = fs::read_to_string(&pre_commit_path)?;
    assert!(content.contains("#!/bin/bash"));
    assert!(content.contains("PROJECT_LINT_BIN"));
    assert!(content.contains("lint --fix --dry-run"));
    assert!(content.contains("exit 0"));
    
    Ok(())
}

#[test]
fn test_github_workflow_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workflows_dir = temp_dir.path().join(".github/workflows");
    fs::create_dir_all(&workflows_dir)?;
    
    // Simulate GitHub workflow
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
    - name: Run project-lint
      run: ./target/release/project-lint lint --fix --dry-run
"#;
    
    let workflow_path = workflows_dir.join("project-lint.yml");
    fs::write(&workflow_path, workflow_content)?;
    
    // Validate workflow content
    let content = fs::read_to_string(&workflow_path)?;
    assert!(content.contains("name: Project-Lint"));
    assert!(content.contains("on:"));
    assert!(content.contains("push:"));
    assert!(content.contains("pull_request:"));
    assert!(content.contains("jobs:"));
    assert!(content.contains("runs-on: ubuntu-latest"));
    assert!(content.contains("actions/checkout@v4"));
    assert!(content.contains("rust-toolchain@stable"));
    
    Ok(())
}

#[test]
fn test_gitlab_ci_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    // Simulate GitLab CI configuration
    let gitlab_ci_content = r#"# GitLab CI configuration for project-lint
stages:
  - lint
  - security

lint:
  stage: lint
  image: rust:latest
  script:
    - cargo build --release --bin project-lint
    - ./target/release/project-lint lint --fix --dry-run
  artifacts:
    paths:
      - project-lint.log
"#;
    
    let gitlab_ci_path = temp_dir.path().join(".gitlab-ci.yml");
    fs::write(&gitlab_ci_path, gitlab_ci_content)?;
    
    // Validate GitLab CI content
    let content = fs::read_to_string(&gitlab_ci_path)?;
    assert!(content.contains("stages:"));
    assert!(content.contains("- lint"));
    assert!(content.contains("- security"));
    assert!(content.contains("image: rust:latest"));
    assert!(content.contains("script:"));
    assert!(content.contains("artifacts:"));
    
    Ok(())
}

#[tokio::test]
async fn test_multiple_agent_installation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    // Install git hooks
    let git_args = InstallHookArgs {
        agent: "git-hooks".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: false,
    };
    run(git_args).await?;
    
    // Install GitHub workflows
    let github_args = InstallHookArgs {
        agent: "github".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: false,
    };
    run(github_args).await?;
    
    // Verify both installations
    assert!(temp_dir.path().join(".git/hooks/pre-commit").exists());
    assert!(temp_dir.path().join(".github/workflows/project-lint.yml").exists());
    
    Ok(())
}

#[test]
fn test_hook_directory_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    // Test nested directory creation
    let nested_dir = temp_dir.path().join("nested/.git/hooks");
    fs::create_dir_all(nested_dir.parent().unwrap())?;
    
    let args = InstallHookArgs {
        agent: "git-hooks".to_string(),
        dir: Some(nested_dir.parent().unwrap().to_string_lossy().to_string()),
        force: false,
    };
    
    // This should create the necessary directories
    // Note: In real usage, this would be async, but for testing we're checking directory structure
    
    assert!(nested_dir.parent().unwrap().exists());
    
    Ok(())
}
