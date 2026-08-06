//! CI/CD parity scanner — verifies pre-commit/CI parity: a shared quality
//! script exists, a CI workflow directory exists, and the justfile defines the
//! standard build targets (clean/build/test/lint/typecheck/fmt).

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;

pub struct CiCdParityScanner {
    standard_targets: Vec<String>,
}

impl CiCdParityScanner {
    pub fn new() -> Self {
        Self {
            standard_targets: vec![
                "clean".to_string(),
                "build".to_string(),
                "test".to_string(),
                "lint".to_string(),
                "typecheck".to_string(),
                "fmt".to_string(),
            ],
        }
    }

    /// Scan a project root for CI/CD parity gaps.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let quality_script = root.join("scripts").join("run-quality-checks.sh");
        if !quality_script.exists() {
            issues.push(ScannerIssue::new(
                "require-quality-script",
                "error",
                "scripts/run-quality-checks.sh",
                "missing shared quality script (shared-quality-scripts.md)",
            ));
        }

        let workflows = root.join(".github").join("workflows");
        if !workflows.exists() {
            issues.push(ScannerIssue::new(
                "require-ci-workflow",
                "warning",
                ".github/workflows/",
                "no CI workflow directory found",
            ));
        }

        let justfile = root.join("justfile");
        if justfile.exists() {
            if let Ok(content) = std::fs::read_to_string(&justfile) {
                for target in &self.standard_targets {
                    // match `target:` at line start (ignoring leading whitespace)
                    let has = content
                        .lines()
                        .any(|l| l.trim_start().starts_with(&format!("{}:", target)));
                    if !has {
                        issues.push(ScannerIssue::new(
                            "standard-build-targets",
                            "warning",
                            "justfile",
                            format!("justfile missing standard target '{}'", target),
                        ));
                    }
                }
            }
        }

        Ok(issues)
    }
}

impl Default for CiCdParityScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flags_missing_quality_script_and_workflows() -> Result<()> {
        let dir = TempDir::new()?;
        let scanner = CiCdParityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "require-quality-script"));
        assert!(issues.iter().any(|i| i.rule == "require-ci-workflow"));
        Ok(())
    }

    #[test]
    fn flags_missing_standard_targets_in_justfile() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("justfile"),
            "build:\n  cargo build\ntest:\n  cargo test\n",
        )?;
        let scanner = CiCdParityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let missing: Vec<String> = issues
            .iter()
            .filter(|i| i.rule == "standard-build-targets")
            .map(|i| i.message.clone())
            .collect();
        assert!(missing.iter().any(|m| m.contains("clean")));
        assert!(missing.iter().any(|m| m.contains("lint")));
        Ok(())
    }

    #[test]
    fn no_target_issues_when_all_present() -> Result<()> {
        let dir = TempDir::new()?;
        let j = "clean:\nbuild:\ntest:\nlint:\ntypecheck:\nfmt:\n";
        std::fs::write(dir.path().join("justfile"), j)?;
        let scanner = CiCdParityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().all(|i| i.rule != "standard-build-targets"));
        Ok(())
    }
}
