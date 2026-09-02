//! Nx config scanner — validates `nx.json` for cache reuse, target defaults,
//! and base branch configuration.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;
use tracing::debug;

pub struct NxConfigScanner {
    require_named_inputs: bool,
    require_target_defaults: bool,
}

impl NxConfigScanner {
    pub fn new() -> Self {
        Self {
            require_named_inputs: true,
            require_target_defaults: true,
        }
    }

    pub fn with_config(require_named_inputs: bool, require_target_defaults: bool) -> Self {
        Self {
            require_named_inputs,
            require_target_defaults,
        }
    }

    /// Scan a project root for `nx.json` and validate its content.
    /// Silent when `nx.json` does not exist.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let nx_json = root.join("nx.json");
        if !nx_json.exists() {
            return Ok(issues);
        }

        let content = match std::fs::read_to_string(&nx_json) {
            Ok(c) => c,
            Err(e) => {
                debug!("Failed to read nx.json: {}", e);
                return Ok(issues);
            }
        };

        let parsed: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                issues.push(ScannerIssue::new(
                    "nx-json-parse",
                    "error",
                    "nx.json",
                    format!("nx.json is not valid JSON: {}", e),
                ));
                return Ok(issues);
            }
        };

        let obj = match parsed.as_object() {
            Some(o) => o,
            None => {
                issues.push(ScannerIssue::new(
                    "nx-json-parse",
                    "error",
                    "nx.json",
                    "nx.json root is not a JSON object",
                ));
                return Ok(issues);
            }
        };

        if self.require_named_inputs && !obj.contains_key("namedInputs") {
            issues.push(ScannerIssue::new(
                "nx-named-inputs",
                "warning",
                "nx.json",
                "nx.json should define 'namedInputs' for cache reuse",
            ));
        }

        if self.require_target_defaults && !obj.contains_key("targetDefaults") {
            issues.push(ScannerIssue::new(
                "nx-target-defaults",
                "warning",
                "nx.json",
                "nx.json should define 'targetDefaults' for standard targets (build, test, lint)",
            ));
        }

        if !obj.contains_key("defaultBase") {
            issues.push(ScannerIssue::new(
                "nx-default-base",
                "info",
                "nx.json",
                "nx.json should define 'defaultBase' (main branch name)",
            ));
        }

        let has_cacheable = obj.contains_key("cacheOperations")
            || obj
                .get("targetDefaults")
                .and_then(|td| td.as_object())
                .map(|td_obj| {
                    td_obj.values().any(|v| {
                        v.as_object()
                            .map(|vo| vo.contains_key("cache"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
        if !has_cacheable {
            issues.push(ScannerIssue::new(
                "nx-cacheable-operations",
                "info",
                "nx.json",
                "nx.json should define 'cacheOperations' or 'targetDefaults' with 'cache: true' for build/test",
            ));
        }

        Ok(issues)
    }
}

impl Default for NxConfigScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_nx_json_absent() -> Result<()> {
        let dir = TempDir::new()?;
        let scanner = NxConfigScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_missing_named_inputs_and_target_defaults() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("nx.json"), r#"{"defaultBase": "main"}"#)?;
        let scanner = NxConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-named-inputs"));
        assert!(issues.iter().any(|i| i.rule == "nx-target-defaults"));
        Ok(())
    }

    #[test]
    fn flags_missing_default_base() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("nx.json"),
            r#"{"namedInputs": {}, "targetDefaults": {}}"#,
        )?;
        let scanner = NxConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-default-base"));
        Ok(())
    }

    #[test]
    fn flags_missing_cacheable_operations() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("nx.json"),
            r#"{"namedInputs": {}, "targetDefaults": {}, "defaultBase": "main"}"#,
        )?;
        let scanner = NxConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-cacheable-operations"));
        Ok(())
    }

    #[test]
    fn clean_nx_json_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("nx.json"),
            r#"{
                "namedInputs": {"default": ["{projectRoot}/**/*"]},
                "targetDefaults": {"build": {"cache": true}},
                "defaultBase": "main"
            }"#,
        )?;
        let scanner = NxConfigScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn cache_operations_satisfies_cacheable_check() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("nx.json"),
            r#"{
                "namedInputs": {"default": ["{projectRoot}/**/*"]},
                "targetDefaults": {},
                "defaultBase": "main",
                "cacheOperations": ["build", "test"]
            }"#,
        )?;
        let scanner = NxConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "nx-cacheable-operations"));
        Ok(())
    }

    #[test]
    fn flags_invalid_json() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("nx.json"), "{not valid json}")?;
        let scanner = NxConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "nx-json-parse" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn config_can_disable_checks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("nx.json"), r#"{"defaultBase": "main"}"#)?;
        let scanner = NxConfigScanner::with_config(false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "nx-named-inputs"));
        assert!(!issues.iter().any(|i| i.rule == "nx-target-defaults"));
        Ok(())
    }

    #[test]
    fn empty_nx_json_flags_all() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("nx.json"), "{}")?;
        let scanner = NxConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "nx-named-inputs"));
        assert!(issues.iter().any(|i| i.rule == "nx-target-defaults"));
        assert!(issues.iter().any(|i| i.rule == "nx-default-base"));
        assert!(issues.iter().any(|i| i.rule == "nx-cacheable-operations"));
        Ok(())
    }
}
