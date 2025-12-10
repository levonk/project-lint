/// pnpm lockfile enforcement rules
/// Implements ADR 20251106001: Use pnpm and Turborepo for Monorepo Management

use regex::Regex;
use std::path::{Path, PathBuf};
use tracing::debug;

pub struct PnpmLockfileRuleSet;

impl PnpmLockfileRuleSet {
    /// Check for forbidden lockfiles
    pub fn check_forbidden_lockfiles(project_path: &Path) -> Result<LockfileValidation, String> {
        let mut violations = Vec::new();

        // Check for npm lockfile
        let npm_lock = project_path.join("package-lock.json");
        if npm_lock.exists() {
            violations.push(LockfileViolation {
                file: "package-lock.json".to_string(),
                package_manager: "npm".to_string(),
                severity: "error".to_string(),
                message: "npm lockfile detected. Use pnpm instead.".to_string(),
            });
        }

        // Check for bun lockfile
        let bun_lock = project_path.join("bun.lock");
        if bun_lock.exists() {
            violations.push(LockfileViolation {
                file: "bun.lock".to_string(),
                package_manager: "bun".to_string(),
                severity: "error".to_string(),
                message: "bun lockfile detected. Use pnpm instead.".to_string(),
            });
        }

        let bun_lockb = project_path.join("bun.lockb");
        if bun_lockb.exists() {
            violations.push(LockfileViolation {
                file: "bun.lockb".to_string(),
                package_manager: "bun".to_string(),
                severity: "error".to_string(),
                message: "bun lockfile detected. Use pnpm instead.".to_string(),
            });
        }

        // Check for yarn lockfile
        let yarn_lock = project_path.join("yarn.lock");
        if yarn_lock.exists() {
            violations.push(LockfileViolation {
                file: "yarn.lock".to_string(),
                package_manager: "yarn".to_string(),
                severity: "error".to_string(),
                message: "yarn lockfile detected. Use pnpm instead.".to_string(),
            });
        }

        // Check for pnpm lockfile
        let pnpm_lock = project_path.join("pnpm-lock.yaml");
        let has_pnpm_lock = pnpm_lock.exists();

        if !has_pnpm_lock && violations.is_empty() {
            violations.push(LockfileViolation {
                file: "pnpm-lock.yaml".to_string(),
                package_manager: "pnpm".to_string(),
                severity: "warn".to_string(),
                message: "pnpm-lock.yaml not found. Run 'pnpm install' to generate it.".to_string(),
            });
        }

        Ok(LockfileValidation {
            has_pnpm_lock,
            violations,
        })
    }

    /// Check package.json scripts for npm/yarn usage
    pub fn check_scripts_for_package_managers(
        package_json_content: &str,
    ) -> Result<Vec<ScriptViolation>, String> {
        let mut violations = Vec::new();

        // Simple regex-based check for npm/yarn commands
        let npm_pattern = Regex::new(r"\bnpm\s+(run|install|test|build)").unwrap();
        let yarn_pattern = Regex::new(r"\byarn\s+(run|install|test|build)").unwrap();

        for (line_num, line) in package_json_content.lines().enumerate() {
            if npm_pattern.is_match(line) {
                violations.push(ScriptViolation {
                    line: line_num + 1,
                    package_manager: "npm".to_string(),
                    message: format!("npm command detected: {}", line.trim()),
                });
            }

            if yarn_pattern.is_match(line) {
                violations.push(ScriptViolation {
                    line: line_num + 1,
                    package_manager: "yarn".to_string(),
                    message: format!("yarn command detected: {}", line.trim()),
                });
            }
        }

        Ok(violations)
    }
}

#[derive(Debug, Clone)]
pub struct LockfileValidation {
    pub has_pnpm_lock: bool,
    pub violations: Vec<LockfileViolation>,
}

#[derive(Debug, Clone)]
pub struct LockfileViolation {
    pub file: String,
    pub package_manager: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ScriptViolation {
    pub line: usize,
    pub package_manager: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_npm_lockfile() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();

        let result = PnpmLockfileRuleSet::check_forbidden_lockfiles(temp_dir.path());
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(!validation.has_pnpm_lock);
        assert_eq!(validation.violations.len(), 1);
        assert_eq!(validation.violations[0].package_manager, "npm");
    }

    #[test]
    fn test_detect_bun_lockfile() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("bun.lock"), "").unwrap();

        let result = PnpmLockfileRuleSet::check_forbidden_lockfiles(temp_dir.path());
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert_eq!(validation.violations.len(), 1);
        assert_eq!(validation.violations[0].package_manager, "bun");
    }

    #[test]
    fn test_detect_pnpm_lockfile() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pnpm-lock.yaml"), "").unwrap();

        let result = PnpmLockfileRuleSet::check_forbidden_lockfiles(temp_dir.path());
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(validation.has_pnpm_lock);
        assert!(validation.violations.is_empty());
    }

    #[test]
    fn test_detect_npm_in_scripts() {
        let content = r#"{
  "scripts": {
    "build": "npm run tsc",
    "test": "npm test"
  }
}"#;

        let result = PnpmLockfileRuleSet::check_scripts_for_package_managers(content);
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_no_violations() {
        let content = r#"{
  "scripts": {
    "build": "tsc",
    "test": "vitest"
  }
}"#;

        let result = PnpmLockfileRuleSet::check_scripts_for_package_managers(content);
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert!(violations.is_empty());
    }
}
