//! Pulumi lint scanner — validates Pulumi.yaml / Pulumi.*.yaml files for
//! required fields (name, runtime), config section presence, secrets in
//! config, and description field.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct PulumiLintScanner {
    require_config: bool,
    forbid_secrets_in_config: bool,
    excluded: Vec<String>,
}

impl PulumiLintScanner {
    pub fn new() -> Self {
        Self {
            require_config: true,
            forbid_secrets_in_config: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_config: bool, forbid_secrets_in_config: bool) -> Self {
        Self {
            require_config,
            forbid_secrets_in_config,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_config: bool,
        forbid_secrets_in_config: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_config,
            forbid_secrets_in_config,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with("Pulumi.") || !name.ends_with(".yaml") {
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
            issues.extend(self.scan_pulumi_file(path, &rel));
        }

        Ok(issues)
    }

    fn scan_pulumi_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut in_config = false;
        let mut config_indent = 0usize;

        let has_name = content.lines().any(|l| {
            let t = l.trim();
            t.starts_with("name:") && !t.starts_with("name::")
        });
        if !has_name {
            issues.push(ScannerIssue::new(
                "pulumi-name-present",
                "error",
                rel,
                "Pulumi.yaml missing 'name:' field",
            ));
        }

        let has_runtime = content.lines().any(|l| {
            let t = l.trim();
            t.starts_with("runtime:")
        });
        if !has_runtime {
            issues.push(ScannerIssue::new(
                "pulumi-runtime-set",
                "error",
                rel,
                "Pulumi.yaml missing 'runtime:' field (language runtime)",
            ));
        }

        let has_description = content.lines().any(|l| {
            let t = l.trim();
            t.starts_with("description:")
        });
        if !has_description {
            issues.push(ScannerIssue::new(
                "pulumi-description-present",
                "info",
                rel,
                "Pulumi.yaml missing 'description:' field",
            ));
        }

        let has_config = content.lines().any(|l| {
            let t = l.trim();
            t == "config:" || t.starts_with("config:")
        });
        if self.require_config && !has_config {
            issues.push(ScannerIssue::new(
                "pulumi-config-present",
                "info",
                rel,
                "Pulumi.yaml missing 'config:' section for environment-specific values",
            ));
        }

        if self.forbid_secrets_in_config {
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed == "config:" || trimmed.starts_with("config:") {
                    in_config = true;
                    config_indent = line.len() - trimmed.len();
                    continue;
                }
                if in_config {
                    let current_indent = line.len() - line.trim_start().len();
                    if !trimmed.is_empty() && current_indent <= config_indent {
                        in_config = false;
                    }
                    if in_config {
                        let lower = trimmed.to_lowercase();
                        if lower.contains("secret")
                            || lower.contains("password")
                            || lower.contains("token")
                            || lower.contains("api_key")
                        {
                            if lower.contains(':') {
                                let value_part = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
                                if value_part.starts_with('"')
                                    && value_part.len() > 2
                                    && !value_part.contains("${")
                                {
                                    issues.push(
                                        ScannerIssue::new(
                                            "pulumi-no-secrets-in-config",
                                            "error",
                                            rel,
                                            "plaintext secret value in config — use 'pulumi config set --secret'",
                                        )
                                        .at_line(i + 1),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        issues
    }
}

impl Default for PulumiLintScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_no_pulumi_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = PulumiLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_missing_name_and_runtime() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Pulumi.yaml"), "description: test\n")?;
        let scanner = PulumiLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pulumi-name-present"));
        assert!(issues.iter().any(|i| i.rule == "pulumi-runtime-set"));
        Ok(())
    }

    #[test]
    fn flags_secret_in_config() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Pulumi.yaml"),
            "name: my-project\nruntime: nodejs\ndescription: test\nconfig:\n  db_password: \"hunter2\"\n",
        )?;
        let scanner = PulumiLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pulumi-no-secrets-in-config"));
        Ok(())
    }

    #[test]
    fn clean_pulumi_yaml_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Pulumi.yaml"),
            "name: my-project\nruntime: nodejs\ndescription: My project\nconfig:\n  region: us-east-1\n",
        )?;
        let scanner = PulumiLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.is_empty(),
            "expected no issues, got: {:?}",
            issues.iter().map(|i| &i.rule).collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn config_can_disable_checks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Pulumi.yaml"),
            "name: my-project\nruntime: nodejs\nconfig:\n  db_password: \"hunter2\"\n",
        )?;
        let scanner = PulumiLintScanner::with_config(false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "pulumi-no-secrets-in-config"));
        Ok(())
    }

    #[test]
    fn empty_pulumi_yaml_flags_required_fields() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Pulumi.yaml"), "")?;
        let scanner = PulumiLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pulumi-name-present"));
        assert!(issues.iter().any(|i| i.rule == "pulumi-runtime-set"));
        Ok(())
    }

    #[test]
    fn handles_pulumi_dev_yaml_variant() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Pulumi.dev.yaml"),
            "name: my-project\nruntime: nodejs\ndescription: test\nconfig:\n  region: us-east-1\n",
        )?;
        let scanner = PulumiLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }
}
