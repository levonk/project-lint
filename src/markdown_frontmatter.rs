/// Markdown frontmatter validation rules
/// Implements ADR 20251106016: Standardized Markdown Frontmatter

use regex::Regex;
use std::path::Path;
use tracing::debug;

pub struct MarkdownFrontmatterRuleSet;

impl MarkdownFrontmatterRuleSet {
    /// Validate markdown file has proper frontmatter
    pub fn validate_frontmatter(content: &str, file_path: &Path) -> Result<FrontmatterValidation, Vec<String>> {
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
                let value = line[colon_pos + 1..].trim();

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
        // Format: YYYYMMDDNNN (14 digits)
        id.len() == 14 && id.chars().all(|c| c.is_ascii_digit())
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

        let result = MarkdownFrontmatterRuleSet::validate_frontmatter(
            content,
            Path::new("test.md"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_title() {
        let content = r#"---
synopsis: "A test document"
tags: ["test"]
---
# Content"#;

        let result = MarkdownFrontmatterRuleSet::validate_frontmatter(
            content,
            Path::new("test.md"),
        );
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
}
