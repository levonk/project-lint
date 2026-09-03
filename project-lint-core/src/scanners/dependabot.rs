//! Dependabot scanner — validates `.github/dependabot.yml` for ecosystem
//! coverage, schedule intervals, group config, assignees/reviewers, and
//! github-actions ecosystem presence when workflows exist.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
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
            issues.push(ScannerIssue::new(
                "dependabot-missing",
                "warning",
                &dependabot_path.to_string_lossy(),
                "Project has no .github/dependabot.yml — dependency updates are not configured. Run with --fix to create one.",
            ));
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

    pub fn apply_fixes(&self, issues: &[ScannerIssue], dry_run: bool) -> Result<usize> {
        let missing = issues.iter().find(|i| i.rule == "dependabot-missing");
        let Some(missing) = missing else {
            return Ok(0);
        };

        if dry_run {
            debug!("dry-run: would create {}", missing.file);
            return Ok(0);
        }

        let dest = Path::new(&missing.file);
        let root = dest
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        let ecosystems = self.detect_ecosystems(root);
        let content = self.generate_config(&ecosystems);

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, content)?;
        debug!("created {}", dest.display());

        Ok(1)
    }

    fn detect_ecosystems(&self, root: &Path) -> Vec<&'static str> {
        let mut ecosystems: Vec<&'static str> = Vec::new();

        if root.join(".github").join("workflows").exists() {
            ecosystems.push("github-actions");
        }
        if root.join("Cargo.toml").exists() {
            ecosystems.push("cargo");
        }
        if root.join("package.json").exists() {
            ecosystems.push("npm");
        }
        if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            ecosystems.push("pip");
        }
        if root.join("Dockerfile").exists() {
            ecosystems.push("docker");
        }

        if ecosystems.is_empty() {
            ecosystems.push("github-actions");
            ecosystems.push("cargo");
        }

        ecosystems
    }

    fn generate_config(&self, ecosystems: &[&str]) -> String {
        let mut out = String::from("version: 2\nupdates:\n");
        for ecosystem in ecosystems {
            out.push_str(&format!(
                "  - package-ecosystem: \"{}\"\n    directory: \"/\"\n    schedule:\n      interval: \"weekly\"\n",
                ecosystem
            ));
        }
        out
    }
}

impl Default for DependabotScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct DependabotFile {
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
    fn missing_file_emits_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "dependabot-missing" && i.severity == "warning"));
        Ok(())
    }

    #[test]
    fn apply_fixes_creates_dependabot_file() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let fixed = scanner.apply_fixes(&issues, false)?;
        assert_eq!(fixed, 1);
        let created = dir.path().join(".github").join("dependabot.yml");
        assert!(created.exists());
        let content = std::fs::read_to_string(&created)?;
        assert!(content.contains("version: 2"));
        assert!(content.contains("cargo"));
        Ok(())
    }

    #[test]
    fn dry_run_does_not_create_file() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let fixed = scanner.apply_fixes(&issues, true)?;
        assert_eq!(fixed, 0);
        assert!(!dir.path().join(".github").join("dependabot.yml").exists());
        Ok(())
    }

    #[test]
    fn generated_config_has_correct_ecosystems() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join(".github").join("workflows"))?;
        std::fs::write(
            dir.path().join(".github").join("workflows").join("ci.yml"),
            "name: CI\n",
        )?;
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n")?;
        std::fs::write(dir.path().join("package.json"), "{}")?;
        std::fs::write(dir.path().join("Dockerfile"), "FROM scratch\n")?;
        std::fs::write(dir.path().join("pyproject.toml"), "[project]\n")?;

        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        scanner.apply_fixes(&issues, false)?;

        let content = std::fs::read_to_string(dir.path().join(".github").join("dependabot.yml"))?;
        assert!(content.contains("github-actions"));
        assert!(content.contains("cargo"));
        assert!(content.contains("npm"));
        assert!(content.contains("pip"));
        assert!(content.contains("docker"));
        Ok(())
    }

    #[test]
    fn generated_config_defaults_when_no_ecosystems_detected() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# empty project\n")?;
        let scanner = DependabotScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        scanner.apply_fixes(&issues, false)?;
        let content = std::fs::read_to_string(dir.path().join(".github").join("dependabot.yml"))?;
        assert!(content.contains("github-actions"));
        assert!(content.contains("cargo"));
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
}
