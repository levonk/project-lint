//! GitHub Actions workflow scanner — validates `.github/workflows/*.yml` files
//! for security and quality: explicit permissions, minimal token scope, no
//! `pull_request_target`, SHA-pinned actions, no secret injection, no `sudo`,
//! valid `runs-on`, concurrency, timeout, and devbox usage.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use serde::Deserialize;
use std::path::Path;
use tracing::debug;

pub struct GithubWorkflowScanner {
    require_permissions: bool,
    require_pinned_actions: bool,
    require_timeout: bool,
    require_devbox: bool,
    forbid_pull_request_target: bool,
    excluded: Vec<String>,
}

impl GithubWorkflowScanner {
    pub fn new() -> Self {
        Self {
            require_permissions: true,
            require_pinned_actions: true,
            require_timeout: true,
            require_devbox: true,
            forbid_pull_request_target: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_permissions: bool,
        require_pinned_actions: bool,
        require_timeout: bool,
        require_devbox: bool,
        forbid_pull_request_target: bool,
    ) -> Self {
        Self {
            require_permissions,
            require_pinned_actions,
            require_timeout,
            require_devbox,
            forbid_pull_request_target,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_permissions: bool,
        require_pinned_actions: bool,
        require_timeout: bool,
        require_devbox: bool,
        forbid_pull_request_target: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_permissions,
            require_pinned_actions,
            require_timeout,
            require_devbox,
            forbid_pull_request_target,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.to_string_lossy();
            if is_excluded_rel(&rel_str, &self.excluded) {
                continue;
            }
            if !rel_str.starts_with(".github/workflows/") {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".yml") && !name.ends_with(".yaml") {
                continue;
            }
            issues.extend(self.scan_workflow(path, &rel_str));
        }

        Ok(issues)
    }

    fn scan_workflow(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        let workflow: WorkflowFile = match serde_yaml::from_str(&content) {
            Ok(w) => w,
            Err(e) => {
                debug!("failed to parse workflow {:?}: {}", path, e);
                issues.push(ScannerIssue::new(
                    "workflow-parse-error",
                    "error",
                    rel,
                    format!("invalid YAML: {}", e),
                ));
                return issues;
            }
        };

        if self.forbid_pull_request_target && workflow.has_pull_request_target() {
            issues.push(ScannerIssue::new(
                "workflow-no-pull-request-target",
                "error",
                rel,
                "workflow uses 'on: pull_request_target' which runs with secrets on fork PRs",
            ));
        }

        if self.require_permissions && workflow.permissions.is_none() {
            issues.push(ScannerIssue::new(
                "workflow-permissions-block",
                "warning",
                rel,
                "workflow missing explicit 'permissions:' block",
            ));
        }

        if let Some(ref perms) = workflow.permissions {
            if perms.has_contents_write() {
                issues.push(ScannerIssue::new(
                    "workflow-permissions-minimal",
                    "warning",
                    rel,
                    "permissions grant 'contents: write'; prefer 'contents: read' unless needed",
                ));
            }
        }

        if self.require_timeout && workflow.timeout_minutes.is_none() {
            issues.push(ScannerIssue::new(
                "workflow-timeout",
                "warning",
                rel,
                "workflow missing 'timeout-minutes:' to prevent hung runs",
            ));
        }

        if workflow.concurrency.is_none() {
            issues.push(ScannerIssue::new(
                "workflow-concurrency",
                "info",
                rel,
                "workflow missing 'concurrency:' to cancel stale runs",
            ));
        }

        if self.require_devbox && !workflow.uses_devbox() {
            issues.push(ScannerIssue::new(
                "workflow-uses-devbox",
                "warning",
                rel,
                "CI workflow does not use 'devbox run --' for build/test commands",
            ));
        }

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }

            if self.require_pinned_actions {
                if let Some(action_ref) = extract_action_ref(trimmed) {
                    if !is_sha_pinned(&action_ref) {
                        issues.push(
                            ScannerIssue::new(
                                "workflow-pinned-actions",
                                "warning",
                                rel,
                                format!(
                                    "action '{}' not pinned by SHA; use @<sha> instead of @<tag>",
                                    action_ref
                                ),
                            )
                            .at_line(i + 1),
                        );
                    }
                }
            }

            if trimmed.contains("sudo ") || trimmed == "sudo" || trimmed.starts_with("sudo ") {
                issues.push(
                    ScannerIssue::new(
                        "workflow-no-sudo",
                        "info",
                        rel,
                        "workflow uses 'sudo' in a step; GitHub runners already have appropriate permissions",
                    )
                    .at_line(i + 1),
                );
            }

            if has_secret_env_injection(trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "workflow-no-inject-secrets",
                        "error",
                        rel,
                        "workflow injects secrets into environment variables that could be logged",
                    )
                    .at_line(i + 1),
                );
            }
        }

        if let Some(ref runs_on) = workflow.runs_on {
            if !is_valid_runs_on(runs_on) {
                issues.push(ScannerIssue::new(
                    "workflow-runs-on-valid",
                    "warning",
                    rel,
                    format!("invalid 'runs-on:' value: {}", runs_on),
                ));
            }
        }

        issues
    }
}

impl Default for GithubWorkflowScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowFile {
    #[serde(default)]
    permissions: Option<PermissionsValue>,
    #[serde(default)]
    runs_on: Option<String>,
    #[serde(default, rename = "timeout-minutes")]
    timeout_minutes: Option<serde_yaml::Value>,
    #[serde(default)]
    concurrency: Option<serde_yaml::Value>,
    #[serde(default, rename = "on")]
    on: Option<serde_yaml::Value>,
    #[serde(default)]
    jobs: Option<serde_yaml::Mapping>,
}

impl WorkflowFile {
    fn has_pull_request_target(&self) -> bool {
        if let Some(ref on_val) = self.on {
            let s = serde_yaml::to_string(on_val).unwrap_or_default();
            s.contains("pull_request_target")
        } else {
            false
        }
    }

    fn uses_devbox(&self) -> bool {
        if let Some(ref jobs) = self.jobs {
            let s = serde_yaml::to_string(jobs).unwrap_or_default();
            s.contains("devbox run")
        } else {
            false
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PermissionsValue {
    String(String),
    Mapping(PermissionsMapping),
}

#[derive(Debug, Default, Deserialize)]
struct PermissionsMapping {
    #[serde(default)]
    contents: Option<String>,
}

impl PermissionsValue {
    fn has_contents_write(&self) -> bool {
        match self {
            PermissionsValue::String(s) => s == "write-all",
            PermissionsValue::Mapping(m) => {
                m.contents.as_ref().map(|c| c == "write").unwrap_or(false)
            }
        }
    }
}

fn extract_action_ref(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("- uses: ") {
        return Some(rest.trim().trim_matches('\'').trim_matches('"').to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("uses: ") {
        return Some(rest.trim().trim_matches('\'').trim_matches('"').to_string());
    }
    None
}

fn is_sha_pinned(action_ref: &str) -> bool {
    if let Some(at_pos) = action_ref.rfind('@') {
        let tag = &action_ref[at_pos + 1..];
        return tag.len() >= 40 && tag.chars().all(|c| c.is_ascii_hexdigit());
    }
    false
}

fn is_valid_runs_on(runs_on: &str) -> bool {
    let valid_labels = [
        "ubuntu-latest",
        "ubuntu-22.04",
        "ubuntu-20.04",
        "macos-latest",
        "macos-13",
        "macos-12",
        "windows-latest",
        "windows-2022",
        "ubuntu-24.04",
        "macos-14",
        "macos-15",
    ];
    if valid_labels.contains(&runs_on) {
        return true;
    }
    if runs_on.starts_with("${{") {
        return true;
    }
    if runs_on.starts_with("self-hosted") {
        return true;
    }
    false
}

fn has_secret_env_injection(line: &str) -> bool {
    if !line.contains("secrets.") {
        return false;
    }
    line.contains("env:") || line.starts_with("TOKEN") || line.starts_with("KEY")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_workflow(dir: &Path, name: &str, content: &str) {
        let wf_dir = dir.join(".github").join("workflows");
        std::fs::create_dir_all(&wf_dir).unwrap();
        std::fs::write(wf_dir.join(name), content).unwrap();
    }

    fn valid_workflow() -> String {
        "name: CI\non:\n  push:\n    branches: [main]\npermissions:\n  contents: read\nconcurrency:\n  group: ci\n  cancel-in-progress: true\ntimeout-minutes: 30\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@abcdef1234567890abcdef1234567890abcdef12\n      - run: devbox run -- just build\n".to_string()
    }

    #[test]
    fn valid_workflow_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(&dir.path(), "ci.yml", &valid_workflow());
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_permissions() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: devbox run -- just build\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "workflow-permissions-block"));
        Ok(())
    }

    #[test]
    fn flags_contents_write() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\npermissions:\n  contents: write\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: devbox run -- just build\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "workflow-permissions-minimal"));
        Ok(())
    }

    #[test]
    fn flags_pull_request_target() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: pull_request_target\npermissions:\n  contents: read\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: devbox run -- just build\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "workflow-no-pull-request-target"));
        Ok(())
    }

    #[test]
    fn flags_unpinned_actions() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\npermissions:\n  contents: read\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: devbox run -- just build\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "workflow-pinned-actions"));
        Ok(())
    }

    #[test]
    fn flags_missing_timeout() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\npermissions:\n  contents: read\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: devbox run -- just build\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "workflow-timeout"));
        Ok(())
    }

    #[test]
    fn flags_missing_devbox_usage() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\npermissions:\n  contents: read\ntimeout-minutes: 30\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo build\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "workflow-uses-devbox"));
        Ok(())
    }

    #[test]
    fn devbox_not_required_when_disabled() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\npermissions:\n  contents: read\ntimeout-minutes: 30\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo build\n",
        );
        let scanner = GithubWorkflowScanner::with_config(true, true, true, false, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "workflow-uses-devbox"));
        Ok(())
    }

    #[test]
    fn flags_sudo_usage() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: push\npermissions:\n  contents: read\ntimeout-minutes: 30\nruns-on: ubuntu-latest\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: sudo apt-get update\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "workflow-no-sudo"));
        Ok(())
    }

    #[test]
    fn silent_when_no_workflows_dir() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = GithubWorkflowScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_invalid_yaml() -> Result<()> {
        let dir = TempDir::new()?;
        write_workflow(
            &dir.path(),
            "ci.yml",
            "name: CI\non: [push\n  bad: yaml: here\n",
        );
        let scanner = GithubWorkflowScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "workflow-parse-error"));
        Ok(())
    }
}
