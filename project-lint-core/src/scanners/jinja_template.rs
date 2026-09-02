//! Jinja2 template lint scanner — validates .j2 / .jinja2 files for secret
//! literal variable names, raw includes, sandbox filters (eval/exec), and
//! absolute paths in include/extends directives.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct JinjaTemplateScanner {
    forbidden_filters: Vec<String>,
    forbid_absolute_paths: bool,
    excluded: Vec<String>,
}

impl JinjaTemplateScanner {
    pub fn new() -> Self {
        Self {
            forbidden_filters: vec!["eval".to_string(), "exec".to_string()],
            forbid_absolute_paths: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(forbidden_filters: Vec<String>, forbid_absolute_paths: bool) -> Self {
        Self {
            forbidden_filters,
            forbid_absolute_paths,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        forbidden_filters: Vec<String>,
        forbid_absolute_paths: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            forbidden_filters,
            forbid_absolute_paths,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".j2") && !name.ends_with(".jinja2") {
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
            issues.extend(self.scan_template(path, &rel));
        }

        Ok(issues)
    }

    fn scan_template(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("{{") && trimmed.contains("}}") {
                let lower = trimmed.to_lowercase();
                if lower.contains("secret_value")
                    || lower.contains("vault_password")
                    || lower.contains("api_secret")
                    || lower.contains("private_key")
                {
                    issues.push(
                        ScannerIssue::new(
                            "jinja-no-secret-literals",
                            "warning",
                            rel,
                            "template references a variable name suggesting a hardcoded secret",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            for filter in &self.forbidden_filters {
                let pipe_filter = format!("| {}", filter);
                let pipe_filter_no_space = format!("|{}", filter);
                if trimmed.contains(&pipe_filter) || trimmed.contains(&pipe_filter_no_space) {
                    issues.push(
                        ScannerIssue::new(
                            "jinja-sandbox-filters",
                            "error",
                            rel,
                            format!(
                                "template uses forbidden filter '{}' — security risk",
                                filter
                            ),
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if self.forbid_absolute_paths {
                if trimmed.contains("{% include")
                    || trimmed.contains("{% extends")
                    || trimmed.contains("{%- include")
                    || trimmed.contains("{%- extends")
                {
                    if let Some(path_val) = extract_template_path(trimmed) {
                        if path_val.starts_with('/') {
                            issues.push(
                                ScannerIssue::new(
                                    "jinja-no-absolute-paths",
                                    "warning",
                                    rel,
                                    format!(
                                        "template include/extends uses absolute path '{}' — use relative",
                                        path_val
                                    ),
                                )
                                .at_line(i + 1),
                            );
                        }
                    }
                }
            }

            if trimmed.contains("{% include")
                || trimmed.contains("{% extends")
                || trimmed.contains("{%- include")
                || trimmed.contains("{%- extends")
            {
            } else if trimmed.contains("{{") && trimmed.contains("include") {
                issues.push(
                    ScannerIssue::new(
                        "jinja-no-raw-include",
                        "info",
                        rel,
                        "raw include via variable — prefer {% include %} / {% extends %} directive",
                    )
                    .at_line(i + 1),
                );
            }
        }

        issues
    }
}

fn extract_template_path(line: &str) -> Option<String> {
    if let Some(start) = line.find('\'') {
        if let Some(end) = line[start + 1..].find('\'') {
            return Some(line[start + 1..start + 1 + end].to_string());
        }
    }
    if let Some(start) = line.find('"') {
        if let Some(end) = line[start + 1..].find('"') {
            return Some(line[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

impl Default for JinjaTemplateScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_no_jinja_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = JinjaTemplateScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_secret_literal_variable() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("template.j2"),
            "password: {{ secret_value }}\n",
        )?;
        let scanner = JinjaTemplateScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "jinja-no-secret-literals"));
        Ok(())
    }

    #[test]
    fn flags_forbidden_filter() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("template.j2"), "{{ code | eval }}\n")?;
        let scanner = JinjaTemplateScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "jinja-sandbox-filters"));
        Ok(())
    }

    #[test]
    fn flags_absolute_path_in_include() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("template.j2"),
            "{% include '/etc/nginx.conf' %}\n",
        )?;
        let scanner = JinjaTemplateScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "jinja-no-absolute-paths"));
        Ok(())
    }

    #[test]
    fn clean_template_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("template.j2"),
            "{% extends 'base.html' %}\n{% block content %}\nHello {{ name }}\n{% endblock %}\n",
        )?;
        let scanner = JinjaTemplateScanner::new();
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
            dir.path().join("template.j2"),
            "{% include '/etc/config' %}\n",
        )?;
        let scanner = JinjaTemplateScanner::with_config(Vec::new(), false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "jinja-no-absolute-paths"));
        Ok(())
    }

    #[test]
    fn empty_template_produces_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("empty.j2"), "")?;
        let scanner = JinjaTemplateScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn handles_jinja2_extension() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("config.jinja2"), "{{ code | exec }}\n")?;
        let scanner = JinjaTemplateScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "jinja-sandbox-filters"));
        Ok(())
    }
}
