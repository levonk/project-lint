//! Dev environment scanner — verifies the mandatory developer UX flow
//! (devbox, direnv, justfile) and flags forbidden tooling files (Makefile).

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;

pub struct DevEnvironmentScanner {
    required_files: Vec<String>,
    forbidden_files: Vec<String>,
}

impl DevEnvironmentScanner {
    pub fn new() -> Self {
        Self {
            required_files: vec![
                "devbox.json".to_string(),
                ".envrc".to_string(),
                "justfile".to_string(),
            ],
            forbidden_files: vec!["Makefile".to_string()],
        }
    }

    pub fn with_files(required: Vec<String>, forbidden: Vec<String>) -> Self {
        Self {
            required_files: required,
            forbidden_files: forbidden,
        }
    }

    /// Scan a project root for missing required files and present forbidden files.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for required in &self.required_files {
            if !root.join(required).exists() {
                issues.push(ScannerIssue::new(
                    "require-dev-file",
                    "error",
                    required,
                    format!("missing required dev environment file '{}'", required),
                ));
            }
        }
        for forbidden in &self.forbidden_files {
            if root.join(forbidden).exists() {
                issues.push(ScannerIssue::new(
                    "forbidden-dev-file",
                    "warning",
                    forbidden,
                    format!(
                        "forbidden dev file '{}' present; prefer justfile",
                        forbidden
                    ),
                ));
            }
        }

        Ok(issues)
    }
}

impl Default for DevEnvironmentScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flags_missing_required_files() -> Result<()> {
        let dir = TempDir::new()?;
        let scanner = DevEnvironmentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<String> = issues.iter().map(|i| i.rule.clone()).collect();
        assert!(rules.iter().any(|r| r == "require-dev-file"));
        assert!(issues.iter().any(|i| i.file == "devbox.json"));
        Ok(())
    }

    #[test]
    fn flags_forbidden_makefile() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Makefile"), "all:\n")?;
        // provide required files so only the forbidden check fires
        std::fs::write(dir.path().join("devbox.json"), "{}")?;
        std::fs::write(dir.path().join(".envrc"), "use flake\n")?;
        std::fs::write(dir.path().join("justfile"), "default:\n")?;
        let scanner = DevEnvironmentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "forbidden-dev-file" && i.file == "Makefile"));
        Ok(())
    }

    #[test]
    fn no_issues_when_all_required_present_and_no_forbidden() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("devbox.json"), "{}")?;
        std::fs::write(dir.path().join(".envrc"), "x\n")?;
        std::fs::write(dir.path().join("justfile"), "x:\n")?;
        let scanner = DevEnvironmentScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }
}
