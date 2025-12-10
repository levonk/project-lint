/// Runtime guards validation rules for browser safety
/// Implements ADR 006: Runtime Guards for Browser-Safe Web Applications

use regex::Regex;
use std::path::Path;
use tracing::debug;

pub struct RuntimeGuardsRuleSet;

impl RuntimeGuardsRuleSet {
    /// Check for unguarded browser API access
    pub fn check_unguarded_browser_access(
        content: &str,
        file_path: &Path,
    ) -> Result<Vec<BrowserAccessViolation>, String> {
        let mut violations = Vec::new();

        // Only check TypeScript/JavaScript files
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let is_ts_file = file_name.ends_with(".ts")
            || file_name.ends_with(".tsx")
            || file_name.ends_with(".mts")
            || file_name.ends_with(".js")
            || file_name.ends_with(".jsx");

        if !is_ts_file {
            return Ok(violations);
        }

        // Check if file has runtime guards import
        let has_guard_import = content.contains("@job-aide/runtime-guards")
            || content.contains("isBrowser")
            || content.contains("assertBrowser")
            || content.contains("assertServer");

        // Patterns for unguarded browser API access
        let patterns = vec![
            (r"typeof\s+window\s*!==\s*['\"]undefined['\"]", "typeof window check"),
            (r"typeof\s+document\s*!==\s*['\"]undefined['\"]", "typeof document check"),
            (r"window\.", "window access"),
            (r"document\.", "document access"),
            (r"navigator\.", "navigator access"),
            (r"localStorage\.", "localStorage access"),
            (r"sessionStorage\.", "sessionStorage access"),
        ];

        for (line_num, line) in content.lines().enumerate() {
            for (pattern, description) in &patterns {
                if let Ok(re) = Regex::new(pattern) {
                    if re.is_match(line) {
                        // Check if line is in a guard check or comment
                        if !Self::is_guarded_line(line) && !has_guard_import {
                            violations.push(BrowserAccessViolation {
                                line: line_num + 1,
                                column: line.find(pattern).unwrap_or(0),
                                api: description.to_string(),
                                message: format!(
                                    "Unguarded browser API access: {}. Import and use @job-aide/runtime-guards",
                                    description
                                ),
                            });
                        }
                    }
                }
            }
        }

        Ok(violations)
    }

    /// Check if runtime guards are properly imported
    pub fn check_runtime_guards_import(content: &str) -> Result<RuntimeGuardsImportValidation, String> {
        let has_import = content.contains("@job-aide/runtime-guards");
        let has_is_browser = content.contains("isBrowser");
        let has_assert_browser = content.contains("assertBrowser");
        let has_assert_server = content.contains("assertServer");

        let guards_used = has_is_browser || has_assert_browser || has_assert_server;

        Ok(RuntimeGuardsImportValidation {
            has_import,
            guards_used,
            is_valid: !guards_used || has_import,
        })
    }

    fn is_guarded_line(line: &str) -> bool {
        let trimmed = line.trim();
        // Check if line is a comment
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            return true;
        }
        // Check if line contains guard function calls
        if trimmed.contains("isBrowser")
            || trimmed.contains("assertBrowser")
            || trimmed.contains("assertServer")
        {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct BrowserAccessViolation {
    pub line: usize,
    pub column: usize,
    pub api: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeGuardsImportValidation {
    pub has_import: bool,
    pub guards_used: bool,
    pub is_valid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_unguarded_window_access() {
        let content = r#"
const el = window.document.getElementById("app");
"#;

        let result = RuntimeGuardsRuleSet::check_unguarded_browser_access(
            content,
            Path::new("test.ts"),
        );
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_guarded_window_access() {
        let content = r#"
import { isBrowser } from "@job-aide/runtime-guards";

if (isBrowser()) {
  const el = window.document.getElementById("app");
}
"#;

        let result = RuntimeGuardsRuleSet::check_unguarded_browser_access(
            content,
            Path::new("test.ts"),
        );
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert!(violations.is_empty());
    }

    #[test]
    fn test_typeof_window_check() {
        let content = r#"
if (typeof window !== "undefined") {
  console.log("browser");
}
"#;

        let result = RuntimeGuardsRuleSet::check_unguarded_browser_access(
            content,
            Path::new("test.ts"),
        );
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_runtime_guards_import_valid() {
        let content = r#"
import { isBrowser } from "@job-aide/runtime-guards";

if (isBrowser()) {
  // safe
}
"#;

        let result = RuntimeGuardsRuleSet::check_runtime_guards_import(content);
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(validation.has_import);
        assert!(validation.guards_used);
        assert!(validation.is_valid);
    }

    #[test]
    fn test_runtime_guards_import_missing() {
        let content = r#"
if (typeof window !== "undefined") {
  // unsafe
}
"#;

        let result = RuntimeGuardsRuleSet::check_runtime_guards_import(content);
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert!(!validation.has_import);
        assert!(!validation.is_valid);
    }

    #[test]
    fn test_non_ts_file() {
        let content = "window.alert('test');";

        let result = RuntimeGuardsRuleSet::check_unguarded_browser_access(
            content,
            Path::new("test.txt"),
        );
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert!(violations.is_empty());
    }
}
