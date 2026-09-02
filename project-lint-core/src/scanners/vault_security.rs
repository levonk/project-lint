//! Vault security scanner — verifies secrets are sourced from environment
//! variables with a required prefix and that no hardcoded secret literals
//! appear in source files. Complements the regex-based `security` scanner with
//! a project-level policy check.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct VaultSecurityScanner {
    required_env_prefix: Option<String>,
    allowed_backends: Vec<String>,
    excluded: Vec<String>,
}

impl VaultSecurityScanner {
    pub fn new() -> Self {
        Self {
            required_env_prefix: None,
            allowed_backends: Vec::new(),
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(required_env_prefix: Option<String>, allowed_backends: Vec<String>) -> Self {
        Self {
            required_env_prefix,
            allowed_backends,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        required_env_prefix: Option<String>,
        allowed_backends: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            required_env_prefix,
            allowed_backends,
            excluded,
        }
    }

    /// Scan a project root for hardcoded secret literals and env-var access
    /// that violates the required prefix policy.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let is_source = name.ends_with(".rs")
                || name.ends_with(".ts")
                || name.ends_with(".js")
                || name.ends_with(".py")
                || name.ends_with(".go");
            if !is_source {
                continue;
            }
            issues.extend(self.scan_source_file(path, &rel));
        }

        Ok(issues)
    }

    fn scan_source_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }
            // hardcoded AKIA-style AWS keys
            if trimmed.contains("AKIA")
                && trimmed
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .count()
                    > 20
            {
                issues.push(
                    ScannerIssue::new(
                        "no-hardcoded-aws-key",
                        "error",
                        rel,
                        "hardcoded AWS access key literal in source",
                    )
                    .at_line(i + 1),
                );
            }
            // env access without required prefix
            if let Some(prefix) = &self.required_env_prefix {
                if let Some(var) = extract_env_var(trimmed) {
                    if !var.starts_with(prefix) {
                        issues.push(
                            ScannerIssue::new(
                                "env-prefix-policy",
                                "warning",
                                rel,
                                format!(
                                    "env var '{}' does not use required prefix '{}'",
                                    var, prefix
                                ),
                            )
                            .at_line(i + 1),
                        );
                    }
                }
            }
        }
        issues
    }
}

/// Best-effort extraction of an env var name from `std::env::var("X")` /
/// `process.env.X` / `os.Getenv("X")` patterns. Returns the variable name.
fn extract_env_var(line: &str) -> Option<String> {
    if let Some(idx) = line.find("env::var(") {
        return extract_quoted(&line[idx..]);
    }
    if let Some(idx) = line.find("process.env.") {
        let rest = &line[idx + "process.env.".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return Some(name);
        }
    }
    if let Some(idx) = line.find("os.Getenv(") {
        return extract_quoted(&line[idx..]);
    }
    None
}

fn extract_quoted(s: &str) -> Option<String> {
    let after = s.split_once('(').map(|x| x.1).unwrap_or(s);
    let after = after.trim_start();
    if after.starts_with('"') {
        let end = after[1..].find('"')?;
        Some(after[1..1 + end].to_string())
    } else if after.starts_with('\'') {
        let end = after[1..].find('\'')?;
        Some(after[1..1 + end].to_string())
    } else {
        None
    }
}

impl Default for VaultSecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flags_hardcoded_aws_key() -> Result<()> {
        let dir = TempDir::new()?;
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src)?;
        std::fs::write(
            src.join("lib.rs"),
            "const KEY: &str = \"AKIAIOSFODNN7EXAMPLE\";\n",
        )?;
        let scanner = VaultSecurityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "no-hardcoded-aws-key"));
        Ok(())
    }

    #[test]
    fn flags_env_var_without_required_prefix() -> Result<()> {
        let dir = TempDir::new()?;
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src)?;
        std::fs::write(
            src.join("lib.rs"),
            "let k = std::env::var(\"API_KEY\").unwrap();\n",
        )?;
        let scanner = VaultSecurityScanner::with_config(Some("APP_".to_string()), vec![]);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "env-prefix-policy" && i.message.contains("API_KEY")));
        Ok(())
    }

    #[test]
    fn no_issue_when_prefix_matches() -> Result<()> {
        let dir = TempDir::new()?;
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src)?;
        std::fs::write(
            src.join("lib.rs"),
            "let k = std::env::var(\"APP_KEY\").unwrap();\n",
        )?;
        let scanner = VaultSecurityScanner::with_config(Some("APP_".to_string()), vec![]);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().all(|i| i.rule != "env-prefix-policy"));
        Ok(())
    }
}
