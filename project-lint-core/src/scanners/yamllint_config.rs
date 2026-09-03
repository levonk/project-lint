//! Yamllint config scanner — checks for a `.yamllint` config file in the
//! project root and offers an auto-fix that writes sensible defaults.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;
use tracing::debug;

pub struct YamllintConfigScanner {
    require_extends: bool,
}

impl YamllintConfigScanner {
    pub fn new() -> Self {
        Self {
            require_extends: false,
        }
    }

    pub fn with_config(require_extends: bool) -> Self {
        Self { require_extends }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let yamllint_path = root.join(".yamllint");
        if !yamllint_path.exists() {
            issues.push(ScannerIssue::new(
                "yamllint-missing-config",
                "warning",
                &yamllint_path.to_string_lossy(),
                "Project has no .yamllint config — YAML style issues won't be caught. Run with --fix to create one.",
            ));
            return Ok(issues);
        }

        let rel = yamllint_path
            .strip_prefix(root)
            .unwrap_or(&yamllint_path)
            .to_string_lossy()
            .to_string();

        if self.require_extends {
            let Ok(content) = std::fs::read_to_string(&yamllint_path) else {
                return Ok(issues);
            };
            if !content
                .lines()
                .any(|l| l.trim_start().starts_with("extends:"))
            {
                issues.push(ScannerIssue::new(
                    "yamllint-missing-extends",
                    "info",
                    &rel,
                    ".yamllint has no 'extends:' field — consider extending a base config",
                ));
            }
        }

        Ok(issues)
    }

    pub fn apply_fixes(&self, issues: &[ScannerIssue], dry_run: bool) -> Result<usize> {
        let missing = issues.iter().find(|i| i.rule == "yamllint-missing-config");
        let Some(missing) = missing else {
            return Ok(0);
        };

        if dry_run {
            debug!("dry-run: would create {}", missing.file);
            return Ok(0);
        }

        let dest = Path::new(&missing.file);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, DEFAULT_YAMLLINT_CONFIG)?;
        debug!("created {}", dest.display());

        Ok(1)
    }
}

impl Default for YamllintConfigScanner {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_YAMLLINT_CONFIG: &str = "---\nextends: default\n\nrules:\n  document-start: disable\n  line-length: disable\n  truthy:\n    check-keys: false\n";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_file_emits_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = YamllintConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "yamllint-missing-config" && i.severity == "warning"));
        Ok(())
    }

    #[test]
    fn apply_fixes_creates_yamllint_file() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = YamllintConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let fixed = scanner.apply_fixes(&issues, false)?;
        assert_eq!(fixed, 1);
        let created = dir.path().join(".yamllint");
        assert!(created.exists());
        let content = std::fs::read_to_string(&created)?;
        assert!(content.contains("extends: default"));
        Ok(())
    }

    #[test]
    fn dry_run_does_not_create_file() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = YamllintConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let fixed = scanner.apply_fixes(&issues, true)?;
        assert_eq!(fixed, 0);
        assert!(!dir.path().join(".yamllint").exists());
        Ok(())
    }

    #[test]
    fn existing_file_no_extends_emits_info_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".yamllint"),
            "rules:\n  line-length: disable\n",
        )?;
        let scanner = YamllintConfigScanner::with_config(true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "yamllint-missing-extends" && i.severity == "info"));
        Ok(())
    }

    #[test]
    fn existing_file_with_extends_no_info() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".yamllint"),
            "extends: default\nrules:\n  line-length: disable\n",
        )?;
        let scanner = YamllintConfigScanner::with_config(true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn existing_file_no_info_when_extends_not_required() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".yamllint"),
            "rules:\n  line-length: disable\n",
        )?;
        let scanner = YamllintConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }
}
