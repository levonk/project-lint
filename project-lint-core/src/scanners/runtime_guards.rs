/// Runtime guards validation rules for browser safety
/// Implements ADR 006: Runtime Guards for Browser-Safe Web Applications
use crate::scanners::ScannerIssue;
use regex::Regex;
use std::path::Path;
use tracing::debug;
use walkdir::WalkDir;

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
            (
                r#"typeof\s+window\s*!==\s*['"]undefined['"]"#,
                "typeof window check",
            ),
            (
                r#"typeof\s+document\s*!==\s*['"]undefined['"]"#,
                "typeof document check",
            ),
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
    pub fn check_runtime_guards_import(
        content: &str,
    ) -> Result<RuntimeGuardsImportValidation, String> {
        let has_import = content.contains("@job-aide/runtime-guards");
        let has_is_browser = content.contains("isBrowser");
        let has_assert_browser = content.contains("assertBrowser");
        let has_assert_server = content.contains("assertServer");
        let has_typeof_window = content.contains("typeof window");

        let guards_used =
            has_is_browser || has_assert_browser || has_assert_server || has_typeof_window;

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

/// Scanner wrapper that walks a project root and checks TypeScript/JavaScript
/// files for unguarded browser API access using the existing
/// `RuntimeGuardsRuleSet` static methods, converting violations to
/// `ScannerIssue` with proper rule names.
pub struct RuntimeGuardsScanner {
    guards_package: String,
    check_extensions: Vec<String>,
}

impl RuntimeGuardsScanner {
    pub fn new() -> Self {
        Self {
            guards_package: "@job-aide/runtime-guards".to_string(),
            check_extensions: vec![
                "ts".to_string(),
                "tsx".to_string(),
                "mts".to_string(),
                "js".to_string(),
                "jsx".to_string(),
            ],
        }
    }

    pub fn with_config(guards_package: String, check_extensions: Vec<String>) -> Self {
        let check_extensions = if check_extensions.is_empty() {
            vec![
                "ts".to_string(),
                "tsx".to_string(),
                "mts".to_string(),
                "js".to_string(),
                "jsx".to_string(),
            ]
        } else {
            check_extensions
        };
        Self {
            guards_package,
            check_extensions,
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
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !self.check_extensions.iter().any(|e| e == &ext) {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };

            let violations = RuntimeGuardsRuleSet::check_unguarded_browser_access(&content, path)
                .map_err(|e| anyhow::anyhow!(e))?;
            for v in violations {
                if let Some(rule) = runtime_guard_rule_for(&v.api) {
                    issues.push(
                        ScannerIssue::new(
                            rule,
                            runtime_guard_severity_for(rule),
                            &rel_str,
                            &v.message,
                        )
                        .at_line(v.line),
                    );
                }
            }
        }

        Ok(issues)
    }
}

impl Default for RuntimeGuardsScanner {
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

fn runtime_guard_rule_for(api: &str) -> Option<&'static str> {
    if api == "typeof window check" {
        Some("runtime-guard-typeof-window")
    } else if api == "typeof document check" {
        Some("runtime-guard-typeof-document")
    } else if api == "window access" {
        Some("runtime-guard-window-access")
    } else if api == "document access" {
        Some("runtime-guard-document-access")
    } else if api == "navigator access" {
        Some("runtime-guard-navigator-access")
    } else if api == "localStorage access" {
        Some("runtime-guard-localstorage-access")
    } else if api == "sessionStorage access" {
        Some("runtime-guard-sessionstorage-access")
    } else {
        None
    }
}

fn runtime_guard_severity_for(rule: &str) -> &'static str {
    match rule {
        "runtime-guard-typeof-window" | "runtime-guard-typeof-document" => "warning",
        _ => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_unguarded_window_access() {
        let content = r#"
const el = window.document.getElementById("app");
"#;

        let result =
            RuntimeGuardsRuleSet::check_unguarded_browser_access(content, Path::new("test.ts"));
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

        let result =
            RuntimeGuardsRuleSet::check_unguarded_browser_access(content, Path::new("test.ts"));
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

        let result =
            RuntimeGuardsRuleSet::check_unguarded_browser_access(content, Path::new("test.ts"));
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

        let result =
            RuntimeGuardsRuleSet::check_unguarded_browser_access(content, Path::new("test.txt"));
        assert!(result.is_ok());

        let violations = result.unwrap();
        assert!(violations.is_empty());
    }

    #[test]
    fn scan_flags_unguarded_window_access() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("app.ts"),
            "const el = window.document.getElementById(\"app\");\n",
        )?;
        let issues = RuntimeGuardsScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "runtime-guard-window-access" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn scan_silent_on_guarded_access() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("app.ts"),
            "import { isBrowser } from \"@job-aide/runtime-guards\";\nif (isBrowser()) { window.alert(); }\n",
        )?;
        let issues = RuntimeGuardsScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_flags_typeof_window_as_warning() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("app.ts"),
            "if (typeof window !== \"undefined\") { console.log(\"browser\"); }\n",
        )?;
        let issues = RuntimeGuardsScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "runtime-guard-typeof-window" && i.severity == "warning"));
        Ok(())
    }

    #[test]
    fn scan_silent_on_empty_project() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let issues = RuntimeGuardsScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_skips_node_modules() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let nm = dir.path().join("node_modules").join("pkg");
        std::fs::create_dir_all(&nm)?;
        std::fs::write(nm.join("bad.ts"), "window.alert('x');\n")?;
        let issues = RuntimeGuardsScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_silent_on_non_ts_files() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "window.alert('x');\n")?;
        let issues = RuntimeGuardsScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }
}
