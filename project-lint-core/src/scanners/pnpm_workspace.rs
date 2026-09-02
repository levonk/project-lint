//! pnpm workspace scanner — validates `pnpm-workspace.yaml` content: packages
//! field presence, glob validity, catalog mode, and no `node_modules` in globs.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;
use tracing::debug;

pub struct PnpmWorkspaceScanner {
    require_catalog: bool,
    check_glob_matches: bool,
}

impl PnpmWorkspaceScanner {
    pub fn new() -> Self {
        Self {
            require_catalog: false,
            check_glob_matches: true,
        }
    }

    pub fn with_config(require_catalog: bool, check_glob_matches: bool) -> Self {
        Self {
            require_catalog,
            check_glob_matches,
        }
    }

    /// Scan a project root for `pnpm-workspace.yaml` and validate its content.
    /// Silent when the file does not exist.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let workspace_file = root.join("pnpm-workspace.yaml");
        let workspace_path = if workspace_file.exists() {
            workspace_file
        } else {
            let alt = root.join("pnpm-workspace.yml");
            if alt.exists() {
                alt
            } else {
                return Ok(issues);
            }
        };

        let rel = workspace_path
            .strip_prefix(root)
            .unwrap_or(&workspace_path)
            .to_string_lossy()
            .to_string();

        let content = match std::fs::read_to_string(&workspace_path) {
            Ok(c) => c,
            Err(e) => {
                debug!("Failed to read {}: {}", rel, e);
                return Ok(issues);
            }
        };

        let parsed: serde_yaml::Value = match serde_yaml::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                issues.push(ScannerIssue::new(
                    "pnpm-workspace-parse",
                    "error",
                    &rel,
                    format!("pnpm-workspace.yaml is not valid YAML: {}", e),
                ));
                return Ok(issues);
            }
        };

        let packages = parsed.get("packages").and_then(|v| v.as_sequence());

        if packages.is_none() {
            issues.push(ScannerIssue::new(
                "pnpm-workspace-packages",
                "error",
                &rel,
                "pnpm-workspace.yaml should have a 'packages:' field with glob patterns",
            ));
        }

        let glob_list: Vec<String> = packages
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if packages.is_some() && glob_list.is_empty() {
            issues.push(ScannerIssue::new(
                "pnpm-workspace-packages",
                "error",
                &rel,
                "pnpm-workspace.yaml 'packages:' field is empty; define at least one glob pattern",
            ));
        }

        for glob in &glob_list {
            if glob.contains("node_modules") {
                issues.push(ScannerIssue::new(
                    "pnpm-workspace-no-node_modules-glob",
                    "error",
                    &rel,
                    format!(
                        "packages glob '{}' includes node_modules; this should be excluded",
                        glob
                    ),
                ));
            }
        }

        if self.check_glob_matches && !glob_list.is_empty() {
            let any_match = glob_list.iter().any(|g| glob_matches_dir(root, g));
            if !any_match {
                issues.push(ScannerIssue::new(
                    "pnpm-workspace-globs-valid",
                    "warning",
                    &rel,
                    "package globs in pnpm-workspace.yaml do not match any directory",
                ));
            }
        }

        if self.require_catalog {
            let has_catalog = parsed.get("catalog").is_some()
                || content.lines().any(|l| l.trim().starts_with("catalog:"));
            if !has_catalog {
                issues.push(ScannerIssue::new(
                    "pnpm-workspace-catalog",
                    "warning",
                    &rel,
                    "catalog mode enabled but no 'catalog:' section in pnpm-workspace.yaml",
                ));
            }
        }

        Ok(issues)
    }
}

impl Default for PnpmWorkspaceScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn glob_matches_dir(root: &Path, glob: &str) -> bool {
    let cleaned = glob.trim_end_matches('/');
    if cleaned.is_empty() {
        return false;
    }
    if cleaned.contains('*') {
        if let Some(slash_pos) = cleaned.rfind('/') {
            let parent = &cleaned[..slash_pos];
            let pattern = &cleaned[slash_pos + 1..];
            let parent_dir = root.join(parent);
            if !parent_dir.is_dir() {
                return false;
            }
            if let Ok(entries) = std::fs::read_dir(&parent_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if glob::Pattern::new(pattern)
                            .map(|p| p.matches(&name_str))
                            .unwrap_or(false)
                        {
                            return true;
                        }
                    }
                }
            }
            false
        } else {
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        if glob::Pattern::new(cleaned)
                            .map(|p| p.matches(&name_str))
                            .unwrap_or(false)
                        {
                            return true;
                        }
                    }
                }
            }
            false
        }
    } else {
        root.join(cleaned).is_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_no_workspace_file() -> Result<()> {
        let dir = TempDir::new()?;
        let scanner = PnpmWorkspaceScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_missing_packages_field() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("pnpm-workspace.yaml"), "only: stuff\n")?;
        let scanner = PnpmWorkspaceScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pnpm-workspace-packages" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn clean_workspace_with_matching_dirs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join("packages").join("foo"))?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'packages/*'\n",
        )?;
        let scanner = PnpmWorkspaceScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_globs_not_matching_any_dir() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'nonexistent/*'\n",
        )?;
        let scanner = PnpmWorkspaceScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pnpm-workspace-globs-valid"));
        Ok(())
    }

    #[test]
    fn flags_node_modules_in_globs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'node_modules/*'\n",
        )?;
        let scanner = PnpmWorkspaceScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pnpm-workspace-no-node_modules-glob" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn flags_missing_catalog_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join("apps").join("web"))?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n",
        )?;
        let scanner = PnpmWorkspaceScanner::with_config(true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pnpm-workspace-catalog"));
        Ok(())
    }

    #[test]
    fn catalog_present_no_issue() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join("apps").join("web"))?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\ncatalog:\n  react: '18.0.0'\n",
        )?;
        let scanner = PnpmWorkspaceScanner::with_config(true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "pnpm-workspace-catalog"));
        Ok(())
    }

    #[test]
    fn flags_invalid_yaml() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages: [unclosed\n",
        )?;
        let scanner = PnpmWorkspaceScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pnpm-workspace-parse" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn yml_extension_also_supported() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join("packages").join("foo"))?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yml"),
            "packages:\n  - 'packages/*'\n",
        )?;
        let scanner = PnpmWorkspaceScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn empty_packages_list_flags_packages_error() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("pnpm-workspace.yaml"), "packages: []\n")?;
        let scanner = PnpmWorkspaceScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pnpm-workspace-packages"));
        Ok(())
    }
}
