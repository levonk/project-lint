//! Nx project scanner — validates `project.json` (Nx) files across a monorepo.
//! Checks that project names match their directory, targets are defined, and
//! tags are present for dependency boundary enforcement.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;
use tracing::debug;

pub struct NxProjectScanner {
    require_tags: bool,
    require_name_matches_dir: bool,
    excluded: Vec<String>,
}

impl NxProjectScanner {
    pub fn new() -> Self {
        Self {
            require_tags: false,
            require_name_matches_dir: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_tags: bool, require_name_matches_dir: bool) -> Self {
        Self {
            require_tags,
            require_name_matches_dir,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_tags: bool,
        require_name_matches_dir: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_tags,
            require_name_matches_dir,
            excluded,
        }
    }

    /// Walk a project for `project.json` files (Nx convention) and validate each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 6).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "project.json" {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }
            issues.extend(self.scan_project_json(path, &rel));
        }

        Ok(issues)
    }

    fn scan_project_json(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                debug!("Failed to read {}: {}", rel, e);
                return issues;
            }
        };

        let parsed: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                issues.push(ScannerIssue::new(
                    "nx-project-parse",
                    "error",
                    rel,
                    format!("project.json is not valid JSON: {}", e),
                ));
                return issues;
            }
        };

        let obj = match parsed.as_object() {
            Some(o) => o,
            None => {
                issues.push(ScannerIssue::new(
                    "nx-project-parse",
                    "error",
                    rel,
                    "project.json root is not a JSON object",
                ));
                return issues;
            }
        };

        if self.require_name_matches_dir {
            if let Some(name_val) = obj.get("name").and_then(|v| v.as_str()) {
                let dir_name = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !dir_name.is_empty() && name_val != dir_name {
                    issues.push(ScannerIssue::new(
                        "nx-project-name-matches-dir",
                        "warning",
                        rel,
                        format!(
                            "project.json name '{}' does not match directory name '{}'",
                            name_val, dir_name
                        ),
                    ));
                }
            }
        }

        let has_targets = obj
            .get("targets")
            .map(|t| t.as_object().map(|o| !o.is_empty()).unwrap_or(false))
            .unwrap_or(false);
        if !has_targets {
            issues.push(ScannerIssue::new(
                "nx-project-has-targets",
                "warning",
                rel,
                "project.json should define at least one target (build, test, lint)",
            ));
        }

        if self.require_tags {
            let has_tags = obj
                .get("tags")
                .map(|t| t.as_array().map(|a| !a.is_empty()).unwrap_or(false))
                .unwrap_or(false);
            if !has_tags {
                issues.push(ScannerIssue::new(
                    "nx-project-tags-present",
                    "info",
                    rel,
                    "project.json should have 'tags' for dependency boundary enforcement",
                ));
            }
        }

        issues
    }
}

impl Default for NxProjectScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_no_project_json() -> Result<()> {
        let dir = TempDir::new()?;
        let scanner = NxProjectScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_name_mismatch() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("packages").join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(
            pkg_dir.join("project.json"),
            r#"{"name": "wrong-name", "targets": {"build": {}}}"#,
        )?;
        let scanner = NxProjectScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "nx-project-name-matches-dir"));
        Ok(())
    }

    #[test]
    fn name_matches_dir_no_issue() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("packages").join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(
            pkg_dir.join("project.json"),
            r#"{"name": "my-app", "targets": {"build": {}}}"#,
        )?;
        let scanner = NxProjectScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "nx-project-name-matches-dir"));
        Ok(())
    }

    #[test]
    fn flags_missing_targets() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(pkg_dir.join("project.json"), r#"{"name": "my-app"}"#)?;
        let scanner = NxProjectScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-project-has-targets"));
        Ok(())
    }

    #[test]
    fn flags_empty_targets() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(
            pkg_dir.join("project.json"),
            r#"{"name": "my-app", "targets": {}}"#,
        )?;
        let scanner = NxProjectScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-project-has-targets"));
        Ok(())
    }

    #[test]
    fn flags_missing_tags_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(
            pkg_dir.join("project.json"),
            r#"{"name": "my-app", "targets": {"build": {}}}"#,
        )?;
        let scanner = NxProjectScanner::with_config(true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-project-tags-present"));
        Ok(())
    }

    #[test]
    fn tags_present_no_issue() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(
            pkg_dir.join("project.json"),
            r#"{"name": "my-app", "targets": {"build": {}}, "tags": ["type:app"]}"#,
        )?;
        let scanner = NxProjectScanner::with_config(true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "nx-project-tags-present"));
        Ok(())
    }

    #[test]
    fn skips_node_modules() -> Result<()> {
        let dir = TempDir::new()?;
        let nm_dir = dir.path().join("node_modules").join("some-pkg");
        std::fs::create_dir_all(&nm_dir)?;
        std::fs::write(
            nm_dir.join("project.json"),
            r#"{"name": "wrong", "targets": {}}"#,
        )?;
        let scanner = NxProjectScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn flags_invalid_json() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(pkg_dir.join("project.json"), "{not valid}")?;
        let scanner = NxProjectScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "nx-project-parse" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn clean_project_json_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let pkg_dir = dir.path().join("my-app");
        std::fs::create_dir_all(&pkg_dir)?;
        std::fs::write(
            pkg_dir.join("project.json"),
            r#"{"name": "my-app", "targets": {"build": {}}, "tags": ["type:app"]}"#,
        )?;
        let scanner = NxProjectScanner::with_config(true, true);
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }
}
