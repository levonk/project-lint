//! Dependabot scanner — validates `.github/dependabot.yml` for ecosystem
//! coverage, schedule intervals, group config, assignees/reviewers, and
//! github-actions ecosystem presence when workflows exist.

use crate::scanners::ScannerIssue;
use crate::utils::{detect_yaml_secrets, Result};
use serde::Deserialize;
use std::path::Path;
use tracing::debug;

pub struct DependabotScanner {
    check_ecosystem_coverage: bool,
    require_group_config: bool,
}

impl DependabotScanner {
    pub fn new() -> Self {
        Self {
            check_ecosystem_coverage: true,
            require_group_config: false,
        }
    }

    pub fn with_config(check_ecosystem_coverage: bool, require_group_config: bool) -> Self {
        Self {
            check_ecosystem_coverage,
            require_group_config,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let dependabot_path = root.join(".github").join("dependabot.yml");
        let dependabot_alt = root.join(".github").join("dependabot.yaml");
        let path = if dependabot_path.exists() {
            dependabot_path
        } else if dependabot_alt.exists() {
            dependabot_alt
        } else {
            return Ok(issues);
        };

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let Ok(content) = std::fs::read_to_string(&path) else {
            return Ok(issues);
        };

        let config: DependabotFile = match serde_yaml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                debug!("failed to parse dependabot.yml: {}", e);
                issues.push(ScannerIssue::new(
                    "dependabot-parse-error",
                    "error",
                    &rel,
                    format!("invalid YAML: {}", e),
                ));
                return Ok(issues);
            }
        };

        for (line, msg) in detect_yaml_secrets(&content) {
            issues
                .push(ScannerIssue::new("yaml-hardcoded-secret", "error", &rel, msg).at_line(line));
        }

        match config.version {
            Some(v) if v != 2 => {
                issues.push(ScannerIssue::new(
                    "dependabot-version-required",
                    "warning",
                    &rel,
                    format!("dependabot.yml 'version:' must be 2, got {}", v),
                ));
            }
            None => {
                issues.push(ScannerIssue::new(
                    "dependabot-version-required",
                    "warning",
                    &rel,
                    "dependabot.yml missing 'version: 2' field",
                ));
            }
            _ => {}
        }

        let entries = config.updates.unwrap_or_default();
        if entries.is_empty() {
            if self.check_ecosystem_coverage {
                issues.push(ScannerIssue::new(
                    "dependabot-ecosystem-coverage",
                    "warning",
                    &rel,
                    "dependabot.yml has no update entries",
                ));
            }
            return Ok(issues);
        }

        let has_workflows = root.join(".github").join("workflows").exists();
        let mut has_github_actions = false;

        for entry in &entries {
            let ecosystem = entry.package_ecosystem.as_deref().unwrap_or("");
            if ecosystem == "github-actions" {
                has_github_actions = true;
            }

            if entry.schedule.is_none()
                || entry
                    .schedule
                    .as_ref()
                    .map(|s| s.interval.is_empty())
                    .unwrap_or(true)
            {
                issues.push(ScannerIssue::new(
                    "dependabot-schedule",
                    "error",
                    &rel,
                    format!("entry for '{}' missing schedule.interval", ecosystem),
                ));
            } else if let Some(ref schedule) = entry.schedule {
                let valid = matches!(schedule.interval.as_str(), "daily" | "weekly" | "monthly");
                if !valid {
                    issues.push(ScannerIssue::new(
                        "dependabot-invalid-interval",
                        "error",
                        &rel,
                        format!(
                            "entry for '{}' has invalid schedule.interval '{}'; must be daily, weekly, or monthly",
                            ecosystem, schedule.interval
                        ),
                    ));
                }
            }

            if self.require_group_config && entry.groups.is_none() {
                issues.push(ScannerIssue::new(
                    "dependabot-group-config",
                    "info",
                    &rel,
                    format!(
                        "entry for '{}' missing 'groups' to batch updates",
                        ecosystem
                    ),
                ));
            }

            if entry.assignees.is_none() && entry.reviewers.is_none() {
                issues.push(ScannerIssue::new(
                    "dependabot-assignees-reviewers",
                    "info",
                    &rel,
                    format!("entry for '{}' has no assignees or reviewers", ecosystem),
                ));
            }
        }

        if has_workflows && !has_github_actions {
            issues.push(ScannerIssue::new(
                "dependabot-actions-ecosystem",
                "warning",
                &rel,
                "project uses GitHub Actions but dependabot.yml has no 'github-actions' ecosystem entry",
            ));
        }

        Ok(issues)
    }
}

impl Default for DependabotScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct DependabotFile {
    #[serde(default)]
    version: Option<u64>,
    #[serde(default, rename = "updates")]
    updates: Option<Vec<DependabotEntry>>,
}

#[derive(Debug, Deserialize)]
struct DependabotEntry {
    #[serde(default, rename = "package-ecosystem")]
    package_ecosystem: Option<String>,
    #[serde(default)]
    schedule: Option<DependabotSchedule>,
    #[serde(default)]
    groups: Option<serde_yaml::Value>,
    #[serde(default)]
    assignees: Option<Vec<String>>,
    #[serde(default)]
    reviewers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct DependabotSchedule {
    #[serde(default)]
    interval: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_dependabot(dir: &Path, content: &str) {
        let gh = dir.join(".github");
        std::fs::create_dir_all(&gh).unwrap();
        std::fs::write(gh.join("dependabot.yml"), content).unwrap();
    }

    fn valid_dependabot() -> String {
        "version: 2\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\n- package-ecosystem: github-actions\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\n".to_string()
    }

    #[test]
    fn valid_dependabot_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(&dir.path(), &valid_dependabot());
        std::fs::create_dir_all(dir.path().join(".github").join("workflows")).unwrap();
        std::fs::write(
            dir.path().join(".github").join("workflows").join("ci.yml"),
            "name: CI\n",
        )?;
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_schedule() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 2\nupdates:\n- package-ecosystem: cargo\n",
        );
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "dependabot-schedule"));
        Ok(())
    }

    #[test]
    fn flags_empty_updates() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(&dir.path(), "version: 2\nupdates: []\n");
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-ecosystem-coverage"));
        Ok(())
    }

    #[test]
    fn flags_missing_actions_ecosystem_when_workflows_exist() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 2\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\n",
        );
        std::fs::create_dir_all(dir.path().join(".github").join("workflows")).unwrap();
        std::fs::write(
            dir.path().join(".github").join("workflows").join("ci.yml"),
            "name: CI\n",
        )?;
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-actions-ecosystem"));
        Ok(())
    }

    #[test]
    fn flags_missing_assignees_reviewers() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 2\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n",
        );
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-assignees-reviewers"));
        Ok(())
    }

    #[test]
    fn group_config_required_when_enabled() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 2\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\n",
        );
        let scanner = DependabotScanner::with_config(true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "dependabot-group-config"));
        Ok(())
    }

    #[test]
    fn silent_when_no_dependabot_file() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = DependabotScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_invalid_yaml() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(&dir.path(), "version: 2\nupdates: [bad: yaml: here\n");
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "dependabot-parse-error"));
        Ok(())
    }

    #[test]
    fn flags_missing_version_field() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "updates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\n",
        );
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-version-required" && i.severity == "warning"));
        Ok(())
    }

    #[test]
    fn flags_wrong_version_value() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 1\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\n",
        );
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-version-required" && i.severity == "warning"));
        Ok(())
    }

    #[test]
    fn flags_invalid_interval() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 2\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: hourly\n  assignees:\n    - levonk\n",
        );
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-invalid-interval" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn flags_hardcoded_secret_in_dependabot() -> Result<()> {
        let dir = TempDir::new()?;
        write_dependabot(
            &dir.path(),
            "version: 2\nupdates:\n- package-ecosystem: cargo\n  schedule:\n    interval: weekly\n  assignees:\n    - levonk\napi_token: abc123\n",
        );
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "yaml-hardcoded-secret" && i.severity == "error"));
        Ok(())
    }
}
