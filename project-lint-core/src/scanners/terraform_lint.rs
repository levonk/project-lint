//! Terraform lint scanner — validates .tf / .tfvars files for resource
//! naming, hardcoded secrets, variable/output descriptions, provider version
//! pinning, backend configuration, default_tags usage, and lockfile presence.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct TerraformLintScanner {
    require_backend: bool,
    require_lockfile: bool,
    forbid_hardcoded_secrets: bool,
    excluded: Vec<String>,
}

impl TerraformLintScanner {
    pub fn new() -> Self {
        Self {
            require_backend: true,
            require_lockfile: true,
            forbid_hardcoded_secrets: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_backend: bool,
        require_lockfile: bool,
        forbid_hardcoded_secrets: bool,
    ) -> Self {
        Self {
            require_backend,
            require_lockfile,
            forbid_hardcoded_secrets,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_backend: bool,
        require_lockfile: bool,
        forbid_hardcoded_secrets: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_backend,
            require_lockfile,
            forbid_hardcoded_secrets,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();
        let mut tf_files: Vec<(std::path::PathBuf, String)> = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".tf") && !name.ends_with(".tfvars") && !name.ends_with(".tf.json") {
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
            tf_files.push((path.to_path_buf(), rel));
        }

        if tf_files.is_empty() {
            return Ok(issues);
        }

        let has_lockfile = root.join(".terraform.lock.hcl").exists();
        let mut project_has_backend = false;

        for (path, rel) in &tf_files {
            let (file_issues, file_has_backend) = self.scan_tf_file(path, rel);
            if file_has_backend {
                project_has_backend = true;
            }
            issues.extend(file_issues);
        }

        if self.require_lockfile && !has_lockfile {
            issues.push(ScannerIssue::new(
                "tf-lockfile-present",
                "warning",
                ".terraform.lock.hcl",
                ".tf files exist but .terraform.lock.hcl is not committed",
            ));
        }

        if self.require_backend && !project_has_backend {
            issues.push(ScannerIssue::new(
                "tf-backend-config",
                "warning",
                "",
                "no 'backend' block found in any terraform configuration (remote state recommended)",
            ));
        }

        Ok(issues)
    }

    fn scan_tf_file(&self, path: &Path, rel: &str) -> (Vec<ScannerIssue>, bool) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return (Vec::new(), false);
        };
        let mut issues = Vec::new();
        let mut has_backend = false;
        let mut in_block = String::new();
        let mut block_start_line = 0usize;
        let mut block_lines: Vec<(usize, String)> = Vec::new();
        let mut brace_depth = 0i32;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if self.forbid_hardcoded_secrets {
                if let Some(issue) = check_hardcoded_secret(trimmed, i + 1, rel) {
                    issues.push(issue);
                }
            }

            if trimmed.starts_with("resource ")
                || trimmed.starts_with("variable ")
                || trimmed.starts_with("output ")
                || trimmed.starts_with("provider ")
                || trimmed.starts_with("terraform ")
            {
                if !in_block.is_empty() {
                    issues.extend(check_block(&in_block, &block_lines, block_start_line, rel));
                }
                in_block = trimmed.to_string();
                block_start_line = i + 1;
                block_lines = vec![(i + 1, line.to_string())];
                brace_depth = 0;
            } else if !in_block.is_empty() {
                block_lines.push((i + 1, line.to_string()));
                brace_depth += trimmed.matches('{').count() as i32;
                brace_depth -= trimmed.matches('}').count() as i32;
                if brace_depth < 0 {
                    issues.extend(check_block(&in_block, &block_lines, block_start_line, rel));
                    if in_block.starts_with("terraform ") {
                        let block_text: String = block_lines
                            .iter()
                            .map(|(_, l)| l.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        if block_text.contains("backend") {
                            has_backend = true;
                        }
                    }
                    in_block.clear();
                    block_lines.clear();
                    brace_depth = 0;
                }
            }
        }

        (issues, has_backend)
    }
}

fn check_hardcoded_secret(trimmed: &str, line: usize, rel: &str) -> Option<ScannerIssue> {
    let secret_patterns = [
        ("password", "password"),
        ("secret_key", "secret_key"),
        ("api_key", "api_key"),
        ("private_key", "private_key"),
        ("access_token", "access_token"),
    ];

    for (keyword, rule_part) in &secret_patterns {
        if trimmed.contains(keyword) {
            if let Some(eq_idx) = trimmed.find('=') {
                let value = trimmed[eq_idx + 1..].trim();
                if value.starts_with('"') && value.len() > 2 && !value.contains("var.") {
                    return Some(
                        ScannerIssue::new(
                            "tf-no-hardcoded-secrets",
                            "error",
                            rel,
                            format!(
                                "hardcoded secret literal for '{}' — use a variable or data source",
                                rule_part
                            ),
                        )
                        .at_line(line),
                    );
                }
            }
        }
    }
    None
}

fn check_block(
    header: &str,
    lines: &[(usize, String)],
    _start_line: usize,
    rel: &str,
) -> Vec<ScannerIssue> {
    let mut issues = Vec::new();
    let block_text: String = lines
        .iter()
        .map(|(_, l)| l.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if header.starts_with("variable ") {
        if !block_text.contains("description") {
            let line = lines.first().map(|(n, _)| *n).unwrap_or(1);
            issues.push(
                ScannerIssue::new(
                    "tf-variable-description",
                    "info",
                    rel,
                    "variable block missing 'description' field",
                )
                .at_line(line),
            );
        }
        if !block_text.contains("type") {
            let line = lines.first().map(|(n, _)| *n).unwrap_or(1);
            issues.push(
                ScannerIssue::new(
                    "tf-variable-type",
                    "warning",
                    rel,
                    "variable block missing 'type' field (implicit typing)",
                )
                .at_line(line),
            );
        }
    }

    if header.starts_with("output ") {
        if !block_text.contains("description") {
            let line = lines.first().map(|(n, _)| *n).unwrap_or(1);
            issues.push(
                ScannerIssue::new(
                    "tf-output-description",
                    "info",
                    rel,
                    "output block missing 'description' field",
                )
                .at_line(line),
            );
        }
    }

    if header.starts_with("provider ") {
        if !block_text.contains("version") {
            let line = lines.first().map(|(n, _)| *n).unwrap_or(1);
            issues.push(
                ScannerIssue::new(
                    "tf-provider-version",
                    "warning",
                    rel,
                    "provider block missing version constraint (use e.g. version = \"~> 3.0\")",
                )
                .at_line(line),
            );
        }
    }

    if header.starts_with("resource ") {
        if let Some(name_part) = header.strip_prefix("resource ") {
            if let Some(quoted_name) = name_part.split('"').nth(1) {
                if quoted_name == "foo"
                    || quoted_name == "bar"
                    || quoted_name == "test"
                    || quoted_name == "example"
                {
                    let line = lines.first().map(|(n, _)| *n).unwrap_or(1);
                    issues.push(
                        ScannerIssue::new(
                            "tf-resource-naming",
                            "warning",
                            rel,
                            format!("resource name '{}' is not descriptive — use snake_case with purpose", quoted_name),
                        )
                        .at_line(line),
                    );
                }
            }
            if block_text.contains("tags =") && !block_text.contains("default_tags") {
                let line = lines
                    .iter()
                    .find(|(_, l)| l.trim().contains("tags ="))
                    .map(|(n, _)| *n)
                    .unwrap_or(1);
                issues.push(
                    ScannerIssue::new(
                        "tf-no-default-tags-in-resource",
                        "info",
                        rel,
                        "per-resource 'tags' found — consider using 'default_tags' in the provider block",
                    )
                    .at_line(line),
                );
            }
        }
    }

    issues
}

impl Default for TerraformLintScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_no_tf_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = TerraformLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_hardcoded_secret() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "resource \"aws_instance\" \"web\" {\n  password = \"hunter2\"\n}\n",
        )?;
        let scanner = TerraformLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "tf-no-hardcoded-secrets"));
        Ok(())
    }

    #[test]
    fn flags_variable_missing_description_and_type() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "variable \"instance_count\" {\n  default = 1\n}\n",
        )?;
        let scanner = TerraformLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "tf-variable-description"));
        assert!(issues.iter().any(|i| i.rule == "tf-variable-type"));
        Ok(())
    }

    #[test]
    fn clean_tf_file_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "terraform {\n  required_providers {\n    aws = {\n      source = \"hashicorp/aws\"\n      version = \"~> 3.0\"\n    }\n  }\n  backend \"s3\" {\n    bucket = \"tf-state\"\n  }\n}\n\nvariable \"instance_count\" {\n  description = \"Number of instances\"\n  type = number\n  default = 1\n}\n\noutput \"instance_ip\" {\n  description = \"The IP of the instance\"\n  value = aws_instance.web.public_ip\n}\n",
        )?;
        std::fs::write(dir.path().join(".terraform.lock.hcl"), "# lockfile\n")?;
        let scanner = TerraformLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.is_empty(),
            "expected no issues, got: {:?}",
            issues.iter().map(|i| &i.rule).collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn flags_missing_lockfile() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "terraform {\n  backend \"s3\" {}\n}\n",
        )?;
        let scanner = TerraformLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "tf-lockfile-present"));
        Ok(())
    }

    #[test]
    fn flags_provider_missing_version() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "terraform {\n  backend \"s3\" {}\n}\nprovider \"aws\" {\n  region = \"us-east-1\"\n}\n",
        )?;
        std::fs::write(dir.path().join(".terraform.lock.hcl"), "")?;
        let scanner = TerraformLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "tf-provider-version"));
        Ok(())
    }

    #[test]
    fn flags_missing_backend() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "resource \"aws_instance\" \"web\" {}\n",
        )?;
        std::fs::write(dir.path().join(".terraform.lock.hcl"), "")?;
        let scanner = TerraformLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "tf-backend-config"));
        Ok(())
    }

    #[test]
    fn config_can_disable_checks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("main.tf"),
            "resource \"aws_instance\" \"web\" {\n  password = \"hunter2\"\n}\n",
        )?;
        let scanner = TerraformLintScanner::with_config(false, false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "tf-no-hardcoded-secrets"));
        assert!(!issues.iter().any(|i| i.rule == "tf-lockfile-present"));
        Ok(())
    }

    #[test]
    fn empty_tf_file_produces_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("main.tf"), "")?;
        std::fs::write(dir.path().join(".terraform.lock.hcl"), "")?;
        let scanner = TerraformLintScanner::with_config(false, true, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }
}
