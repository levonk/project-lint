/// Markdown frontmatter validation rules
/// Implements ADR 20251106016: Standardized Markdown Frontmatter
use crate::scanners::ScannerIssue;
use regex::Regex;
use std::path::Path;
use tracing::debug;
use walkdir::WalkDir;

pub struct MarkdownFrontmatterRuleSet;

impl MarkdownFrontmatterRuleSet {
    /// Validate markdown file has proper frontmatter
    pub fn validate_frontmatter(
        content: &str,
        file_path: &Path,
    ) -> Result<FrontmatterValidation, Vec<String>> {
        let mut errors = Vec::new();

        // Check if file starts with frontmatter
        if !content.starts_with("---") {
            errors.push("Missing frontmatter block (must start with ---)".to_string());
            return Err(errors);
        }

        // Extract frontmatter block
        let frontmatter_end = content[3..].find("---").map(|i| i + 3);
        if frontmatter_end.is_none() {
            errors.push("Incomplete frontmatter block (missing closing ---)".to_string());
            return Err(errors);
        }

        let frontmatter_block = &content[3..frontmatter_end.unwrap()];

        // Parse YAML fields (simple key: value parsing)
        let mut fields = FrontmatterFields::default();
        let mut has_title = false;
        let mut has_synopsis = false;
        let mut has_tags = false;

        for line in frontmatter_block.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim();
                let value = Self::strip_quotes(line[colon_pos + 1..].trim());

                match key {
                    "title" => {
                        has_title = true;
                        if value.is_empty() {
                            errors.push("title field is empty".to_string());
                        }
                    }
                    "synopsis" => {
                        has_synopsis = true;
                        if value.is_empty() {
                            errors.push("synopsis field is empty".to_string());
                        }
                    }
                    "tags" => {
                        has_tags = true;
                        if value.is_empty() || value == "[]" {
                            errors.push("tags array is empty".to_string());
                        }
                    }
                    "adr-id" => {
                        fields.adr_id = Some(value.to_string());
                        // Validate format YYYYMMDDNNN
                        if !Self::is_valid_adr_id(value) {
                            errors.push(format!(
                                "Invalid adr-id format '{}'. Expected YYYYMMDDNNN",
                                value
                            ));
                        }
                    }
                    "status" => {
                        fields.status = Some(value.to_string());
                        if !["proposed", "accepted", "deprecated", "superseded"].contains(&value) {
                            errors.push(format!(
                                "Invalid status '{}'. Must be: proposed, accepted, deprecated, or superseded",
                                value
                            ));
                        }
                    }
                    "date-created" | "date-updated" => {
                        if !Self::is_valid_date(value) {
                            errors.push(format!(
                                "Invalid date format '{}'. Expected YYYY-MM-DD",
                                value
                            ));
                        }
                    }
                    "version" => {
                        if !Self::is_valid_semver(value) {
                            errors.push(format!(
                                "Invalid version format '{}'. Expected semantic versioning",
                                value
                            ));
                        }
                    }
                    _ => {
                        // Unknown fields are allowed
                    }
                }
            }
        }

        // Check required fields
        if !has_title {
            errors.push("Missing required field: title".to_string());
        }
        if !has_synopsis {
            errors.push("Missing required field: synopsis".to_string());
        }
        if !has_tags {
            errors.push("Missing required field: tags".to_string());
        }

        // Check if this is an ADR file
        if file_path.to_string_lossy().contains("internal-docs/adr") {
            if fields.adr_id.is_none() {
                errors.push("ADR files must have adr-id field".to_string());
            }
            if fields.status.is_none() {
                errors.push("ADR files must have status field".to_string());
            }
        }

        if errors.is_empty() {
            Ok(FrontmatterValidation {
                is_valid: true,
                fields,
            })
        } else {
            Err(errors)
        }
    }

    fn is_valid_adr_id(id: &str) -> bool {
        // Format: YYYYMMDDNNN (11 digits)
        id.len() == 11 && id.chars().all(|c| c.is_ascii_digit())
    }

    /// Strip a single matching pair of surrounding quotes (`"` or `'`) from a
    /// YAML scalar value. Handles `"value"`, `'value'`, and bare `value`.
    fn strip_quotes(value: &str) -> &str {
        let bytes = value.as_bytes();
        if bytes.len() >= 2
            && (bytes[0] == b'"' || bytes[0] == b'\'')
            && bytes[0] == bytes[bytes.len() - 1]
        {
            &value[1..value.len() - 1]
        } else {
            value
        }
    }

    fn is_valid_date(date: &str) -> bool {
        // Format: YYYY-MM-DD
        let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        re.is_match(date)
    }

    fn is_valid_semver(version: &str) -> bool {
        // Format: X.Y.Z
        let re = Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
        re.is_match(version)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FrontmatterFields {
    pub adr_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrontmatterValidation {
    pub is_valid: bool,
    pub fields: FrontmatterFields,
}

/// Scanner wrapper that walks a project root and validates markdown frontmatter
/// in all `.md` files using the existing `MarkdownFrontmatterRuleSet` static
/// methods, converting errors to `ScannerIssue` with proper rule names.
pub struct MarkdownFrontmatterScanner {
    require_frontmatter: bool,
    adr_dirs: Vec<String>,
}

impl MarkdownFrontmatterScanner {
    pub fn new() -> Self {
        Self {
            require_frontmatter: false,
            adr_dirs: vec![
                "internal-docs/adr".to_string(),
                "docs-internal/adr".to_string(),
            ],
        }
    }

    pub fn with_config(require_frontmatter: bool, adr_dirs: Vec<String>) -> Self {
        let adr_dirs = if adr_dirs.is_empty() {
            vec![
                "internal-docs/adr".to_string(),
                "docs-internal/adr".to_string(),
            ]
        } else {
            adr_dirs
        };
        Self {
            require_frontmatter,
            adr_dirs,
        }
    }

    pub fn scan(&self, project_path: &str) -> anyhow::Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in WalkDir::new(root)
            .max_depth(6)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.to_string_lossy();
            if is_excluded_path(&rel_str) {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !name.ends_with(".md") {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };

            let is_adr = self.is_adr_file(&rel_str);

            if !content.starts_with("---") {
                if self.require_frontmatter {
                    issues.push(ScannerIssue::new(
                        "md-frontmatter-present",
                        "warning",
                        &rel_str,
                        "Missing frontmatter block (must start with ---)",
                    ));
                }
                continue;
            }

            if !content[3..].contains("---") {
                issues.push(ScannerIssue::new(
                    "md-frontmatter-closed",
                    "error",
                    &rel_str,
                    "Incomplete frontmatter block (missing closing ---)",
                ));
                continue;
            }

            match MarkdownFrontmatterRuleSet::validate_frontmatter(&content, path) {
                Ok(_) => {}
                Err(errors) => {
                    for err in errors {
                        if let Some(rule) = frontmatter_rule_for(&err, is_adr) {
                            issues.push(ScannerIssue::new(
                                rule,
                                frontmatter_severity_for(rule),
                                &rel_str,
                                &err,
                            ));
                        }
                    }
                }
            }
        }

        Ok(issues)
    }

    fn is_adr_file(&self, rel: &str) -> bool {
        self.adr_dirs
            .iter()
            .any(|dir| rel.starts_with(dir.as_str()))
    }
}

impl Default for MarkdownFrontmatterScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn is_excluded_path(rel: &str) -> bool {
    let segments: Vec<&str> = rel.split('/').collect();
    segments.iter().any(|seg| {
        matches!(
            *seg,
            "node_modules" | "target" | "dist" | ".next" | ".turbo" | ".git"
        )
    })
}

fn frontmatter_severity_for(rule: &str) -> &'static str {
    match rule {
        "md-frontmatter-closed" => "error",
        "adr-id-required"
        | "adr-id-format"
        | "adr-status-required"
        | "adr-status-valid"
        | "adr-date-format"
        | "adr-version-format" => "error",
        _ => "warning",
    }
}

fn frontmatter_rule_for(msg: &str, is_adr: bool) -> Option<&'static str> {
    if msg.contains("Missing frontmatter block") {
        Some("md-frontmatter-present")
    } else if msg.contains("missing closing") {
        Some("md-frontmatter-closed")
    } else if msg.contains("Missing required field: title") || msg.contains("title field is empty")
    {
        Some("md-frontmatter-title")
    } else if msg.contains("Missing required field: synopsis")
        || msg.contains("synopsis field is empty")
    {
        Some("md-frontmatter-synopsis")
    } else if msg.contains("Missing required field: tags") || msg.contains("tags array is empty") {
        Some("md-frontmatter-tags")
    } else if is_adr && msg.contains("ADR files must have adr-id") {
        Some("adr-id-required")
    } else if msg.contains("Invalid adr-id format") {
        Some("adr-id-format")
    } else if is_adr && msg.contains("ADR files must have status") {
        Some("adr-status-required")
    } else if msg.contains("Invalid status") {
        Some("adr-status-valid")
    } else if msg.contains("Invalid date format") {
        Some("adr-date-format")
    } else if msg.contains("Invalid version format") {
        Some("adr-version-format")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_frontmatter() {
        let content = r#"---
title: "Test Document"
synopsis: "A test document"
tags: ["test", "example"]
---
# Content"#;

        let result =
            MarkdownFrontmatterRuleSet::validate_frontmatter(content, Path::new("test.md"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_title() {
        let content = r#"---
synopsis: "A test document"
tags: ["test"]
---
# Content"#;

        let result =
            MarkdownFrontmatterRuleSet::validate_frontmatter(content, Path::new("test.md"));
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("title")));
    }

    #[test]
    fn test_valid_adr_id() {
        assert!(MarkdownFrontmatterRuleSet::is_valid_adr_id("20251126001"));
        assert!(!MarkdownFrontmatterRuleSet::is_valid_adr_id("2025112600"));
        assert!(!MarkdownFrontmatterRuleSet::is_valid_adr_id("202511260001"));
    }

    #[test]
    fn test_valid_date() {
        assert!(MarkdownFrontmatterRuleSet::is_valid_date("2025-11-26"));
        assert!(!MarkdownFrontmatterRuleSet::is_valid_date("2025/11/26"));
        assert!(!MarkdownFrontmatterRuleSet::is_valid_date("11-26-2025"));
    }

    #[test]
    fn test_valid_semver() {
        assert!(MarkdownFrontmatterRuleSet::is_valid_semver("1.0.0"));
        assert!(MarkdownFrontmatterRuleSet::is_valid_semver("2.3.4"));
        assert!(!MarkdownFrontmatterRuleSet::is_valid_semver("1.0"));
        assert!(!MarkdownFrontmatterRuleSet::is_valid_semver("1.0.0.0"));
    }

    #[test]
    fn test_adr_file_validation() {
        let content = r#"---
title: "Test ADR"
synopsis: "A test ADR"
tags: ["adr"]
adr-id: "20251126001"
status: "accepted"
date-created: "2025-11-26"
date-updated: "2025-11-26"
version: "1.0.0"
---
# Content"#;

        let result = MarkdownFrontmatterRuleSet::validate_frontmatter(
            content,
            Path::new("internal-docs/adr/test.md"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn scan_flags_missing_title() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("test.md"),
            "---\nsynopsis: \"A doc\"\ntags: [\"x\"]\n---\n# Body\n",
        )?;
        let issues = MarkdownFrontmatterScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "md-frontmatter-title"));
        Ok(())
    }

    #[test]
    fn scan_silent_on_valid_frontmatter() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("test.md"),
            "---\ntitle: \"Doc\"\nsynopsis: \"A doc\"\ntags: [\"x\"]\n---\n# Body\n",
        )?;
        let issues = MarkdownFrontmatterScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_silent_on_no_frontmatter_by_default() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(dir.path().join("test.md"), "# No frontmatter\n")?;
        let issues = MarkdownFrontmatterScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_flags_missing_frontmatter_when_required() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(dir.path().join("test.md"), "# No frontmatter\n")?;
        let scanner = MarkdownFrontmatterScanner::with_config(true, Vec::new());
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "md-frontmatter-present" && i.severity == "warning"));
        Ok(())
    }

    #[test]
    fn scan_flags_incomplete_frontmatter() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("test.md"),
            "---\ntitle: \"Doc\"\nsynopsis: \"A doc\"\ntags: [\"x\"]\n# no closing delimiter\n",
        )?;
        let issues = MarkdownFrontmatterScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "md-frontmatter-closed" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn scan_silent_on_empty_project() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let issues = MarkdownFrontmatterScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_skips_node_modules() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let nm = dir.path().join("node_modules").join("pkg");
        std::fs::create_dir_all(&nm)?;
        std::fs::write(
            nm.join("bad.md"),
            "---\nsynopsis: \"x\"\ntags: [\"y\"]\n---\n",
        )?;
        let issues = MarkdownFrontmatterScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }
}
