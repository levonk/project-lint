/// Configuration file validation rules
/// Validates tsconfig.json, eslint.config.mts, tailwind.config.ts, package.json
use crate::scanners::ScannerIssue;
use regex::Regex;
use std::path::Path;
use tracing::debug;
use walkdir::WalkDir;

pub struct ConfigValidationRuleSet;

impl ConfigValidationRuleSet {
    /// Validate tsconfig.json
    pub fn validate_tsconfig(content: &str) -> Result<Vec<ConfigViolation>, String> {
        let mut violations = Vec::new();

        // Check for strict mode
        if !content.contains(r#""strict": true"#) && !content.contains(r#"'strict': true"#) {
            violations.push(ConfigViolation {
                file: "tsconfig.json".to_string(),
                severity: "high".to_string(),
                message: "TypeScript strict mode not enabled. Add \"strict\": true".to_string(),
            });
        }

        // Check for module resolution
        if !content.contains("moduleResolution") {
            violations.push(ConfigViolation {
                file: "tsconfig.json".to_string(),
                severity: "medium".to_string(),
                message: "moduleResolution not configured. Recommended: \"bundler\" or \"node\""
                    .to_string(),
            });
        }

        // Check for ambiguous path aliases
        if content.contains(r#""@/*""#) || content.contains(r#"'@/*'"#) {
            violations.push(ConfigViolation {
                file: "tsconfig.json".to_string(),
                severity: "high".to_string(),
                message: "Ambiguous path alias @/* detected. Use explicit aliases like @/core/*, @/features/*".to_string(),
            });
        }

        // Check for rootDir and outDir
        if !content.contains(r#""rootDir""#) && !content.contains(r#"'rootDir'"#) {
            violations.push(ConfigViolation {
                file: "tsconfig.json".to_string(),
                severity: "medium".to_string(),
                message: "rootDir not configured. Recommended: \"./src\"".to_string(),
            });
        }

        if !content.contains(r#""outDir""#) && !content.contains(r#"'outDir'"#) {
            violations.push(ConfigViolation {
                file: "tsconfig.json".to_string(),
                severity: "medium".to_string(),
                message: "outDir not configured. Recommended: \"./dist\"".to_string(),
            });
        }

        Ok(violations)
    }

    /// Validate eslint.config.mts
    pub fn validate_eslint_config(
        content: &str,
        file_name: &str,
    ) -> Result<Vec<ConfigViolation>, String> {
        let mut violations = Vec::new();

        // Check file extension
        if file_name != "eslint.config.mts" {
            violations.push(ConfigViolation {
                file: file_name.to_string(),
                severity: "high".to_string(),
                message: "ESLint config must be named eslint.config.mts (not .ts or .js)"
                    .to_string(),
            });
        }

        // Check for @job-aide/tools-lint-eslint-config
        if !content.contains("@job-aide/tools-lint-eslint-config") {
            violations.push(ConfigViolation {
                file: "eslint.config.mts".to_string(),
                severity: "high".to_string(),
                message: "Must use @job-aide/tools-lint-eslint-config as base config".to_string(),
            });
        }

        // Check for runtime guards plugin in web projects
        if content.contains("react: true") && !content.contains("require-browser-guard") {
            violations.push(ConfigViolation {
                file: "eslint.config.mts".to_string(),
                severity: "medium".to_string(),
                message: "Web project should include runtime guards plugin for browser safety"
                    .to_string(),
            });
        }

        Ok(violations)
    }

    /// Validate tailwind.config.ts
    pub fn validate_tailwind_config(
        content: &str,
        file_name: &str,
    ) -> Result<Vec<ConfigViolation>, String> {
        let mut violations = Vec::new();

        // Check file extension
        if !file_name.ends_with(".ts") && !file_name.ends_with(".mts") {
            violations.push(ConfigViolation {
                file: file_name.to_string(),
                severity: "high".to_string(),
                message: "Tailwind config must be .ts or .mts (not .js)".to_string(),
            });
        }

        // Check for content configuration
        if !content.contains("content:") && !content.contains("content :") {
            violations.push(ConfigViolation {
                file: file_name.to_string(),
                severity: "high".to_string(),
                message:
                    "Tailwind content configuration missing. Add content array with file patterns"
                        .to_string(),
            });
        }

        // Check if content is empty
        if content.contains("content: []") || content.contains("content: [ ]") {
            violations.push(ConfigViolation {
                file: file_name.to_string(),
                severity: "high".to_string(),
                message: "Tailwind content array is empty. Add file patterns for purging"
                    .to_string(),
            });
        }

        Ok(violations)
    }

    /// Validate package.json
    pub fn validate_package_json(content: &str) -> Result<Vec<ConfigViolation>, String> {
        let mut violations = Vec::new();

        // Check for type field
        if !content.contains(r#""type""#) && !content.contains(r#"'type'"#) {
            violations.push(ConfigViolation {
                file: "package.json".to_string(),
                severity: "high".to_string(),
                message: "Missing \"type\" field. Add \"type\": \"module\" for ESM packages"
                    .to_string(),
            });
        }

        // Check for exports field in libraries
        if content.contains(r#""name""#) && !content.contains(r#""exports""#) {
            violations.push(ConfigViolation {
                file: "package.json".to_string(),
                severity: "medium".to_string(),
                message: "Missing \"exports\" field. Recommended for library packages".to_string(),
            });
        }

        // Check for npm/yarn commands in scripts
        if content.contains("npm run") || content.contains("npm install") {
            violations.push(ConfigViolation {
                file: "package.json".to_string(),
                severity: "high".to_string(),
                message: "npm commands detected in scripts. Use pnpm instead".to_string(),
            });
        }

        if content.contains("yarn ") {
            violations.push(ConfigViolation {
                file: "package.json".to_string(),
                severity: "high".to_string(),
                message: "yarn commands detected in scripts. Use pnpm instead".to_string(),
            });
        }

        Ok(violations)
    }
}

#[derive(Debug, Clone)]
pub struct ConfigViolation {
    pub file: String,
    pub severity: String,
    pub message: String,
}

/// Scanner wrapper that walks a project root and validates configuration files
/// (tsconfig.json, eslint.config.*, tailwind.config.*, package.json) using the
/// existing `ConfigValidationRuleSet` static methods, converting violations to
/// `ScannerIssue` with proper rule names.
pub struct ConfigValidationScanner {
    required_eslint_base: Option<String>,
    require_type_module: bool,
    check_tailwind: bool,
}

impl ConfigValidationScanner {
    pub fn new() -> Self {
        Self {
            required_eslint_base: Some("@job-aide/tools-lint-eslint-config".to_string()),
            require_type_module: true,
            check_tailwind: true,
        }
    }

    pub fn with_config(
        required_eslint_base: Option<String>,
        require_type_module: bool,
        check_tailwind: bool,
    ) -> Self {
        Self {
            required_eslint_base,
            require_type_module,
            check_tailwind,
        }
    }

    pub fn scan(&self, project_path: &str) -> anyhow::Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in WalkDir::new(root)
            .max_depth(4)
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

            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };

            if name == "tsconfig.json" {
                let violations = ConfigValidationRuleSet::validate_tsconfig(&content)
                    .map_err(|e| anyhow::anyhow!(e))?;
                for v in violations {
                    if let Some(rule) = tsconfig_rule_for(&v.message) {
                        issues.push(ScannerIssue::new(
                            rule,
                            severity_from(&v.severity),
                            &rel_str,
                            &v.message,
                        ));
                    }
                }
            } else if name.starts_with("eslint.config.") {
                let violations = ConfigValidationRuleSet::validate_eslint_config(&content, &name)
                    .map_err(|e| anyhow::anyhow!(e))?;
                for v in violations {
                    if let Some(rule) = eslint_rule_for(&v.message) {
                        if rule == "eslint-config-base"
                            && self
                                .required_eslint_base
                                .as_deref()
                                .is_none_or(|s| s.is_empty())
                        {
                            continue;
                        }
                        issues.push(ScannerIssue::new(
                            rule,
                            severity_from(&v.severity),
                            &rel_str,
                            &v.message,
                        ));
                    }
                }
            } else if self.check_tailwind && (name.starts_with("tailwind.config.")) {
                let violations = ConfigValidationRuleSet::validate_tailwind_config(&content, &name)
                    .map_err(|e| anyhow::anyhow!(e))?;
                for v in violations {
                    if let Some(rule) = tailwind_rule_for(&v.message) {
                        issues.push(ScannerIssue::new(
                            rule,
                            severity_from(&v.severity),
                            &rel_str,
                            &v.message,
                        ));
                    }
                }
            } else if name == "package.json" {
                let violations = ConfigValidationRuleSet::validate_package_json(&content)
                    .map_err(|e| anyhow::anyhow!(e))?;
                for v in violations {
                    if let Some(rule) = package_json_rule_for(&v.message) {
                        if rule == "package-json-type-field" && !self.require_type_module {
                            continue;
                        }
                        issues.push(ScannerIssue::new(
                            rule,
                            severity_from(&v.severity),
                            &rel_str,
                            &v.message,
                        ));
                    }
                }
            }
        }

        Ok(issues)
    }
}

impl Default for ConfigValidationScanner {
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

fn severity_from(raw: &str) -> &str {
    match raw {
        "high" => "error",
        "medium" => "warning",
        "low" => "info",
        _ => "warning",
    }
}

fn tsconfig_rule_for(msg: &str) -> Option<&'static str> {
    if msg.contains("strict mode") {
        Some("tsconfig-strict-mode")
    } else if msg.contains("moduleResolution") {
        Some("tsconfig-module-resolution")
    } else if msg.contains("@/*") {
        Some("tsconfig-no-ambiguous-alias")
    } else if msg.contains("rootDir") {
        Some("tsconfig-rootdir")
    } else if msg.contains("outDir") {
        Some("tsconfig-outdir")
    } else {
        None
    }
}

fn eslint_rule_for(msg: &str) -> Option<&'static str> {
    if msg.contains("eslint.config.mts") {
        Some("eslint-config-extension")
    } else if msg.contains("tools-lint-eslint-config") {
        Some("eslint-config-base")
    } else if msg.contains("runtime guards plugin") {
        Some("eslint-runtime-guards-plugin")
    } else {
        None
    }
}

fn tailwind_rule_for(msg: &str) -> Option<&'static str> {
    if msg.contains("must be .ts or .mts") {
        Some("tailwind-config-extension")
    } else if msg.contains("content configuration missing") {
        Some("tailwind-content-present")
    } else if msg.contains("content array is empty") {
        Some("tailwind-content-not-empty")
    } else {
        None
    }
}

fn package_json_rule_for(msg: &str) -> Option<&'static str> {
    if msg.contains("\"type\"") {
        Some("package-json-type-field")
    } else if msg.contains("\"exports\"") {
        Some("package-json-exports-field")
    } else if msg.contains("npm commands") {
        Some("package-json-no-npm-scripts")
    } else if msg.contains("yarn commands") {
        Some("package-json-no-yarn-scripts")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsconfig_strict_mode() {
        let content = r#"{ "compilerOptions": { "strict": true } }"#;
        let result = ConfigValidationRuleSet::validate_tsconfig(content);
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().all(|v| !v.message.contains("strict")));
    }

    #[test]
    fn test_tsconfig_missing_strict() {
        let content = r#"{ "compilerOptions": {} }"#;
        let result = ConfigValidationRuleSet::validate_tsconfig(content);
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains("strict")));
    }

    #[test]
    fn test_tsconfig_ambiguous_alias() {
        let content = r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*"] } } }"#;
        let result = ConfigValidationRuleSet::validate_tsconfig(content);
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains("@/*")));
    }

    #[test]
    fn test_eslint_config_extension() {
        let content = "export default {}";
        let result = ConfigValidationRuleSet::validate_eslint_config(content, "eslint.config.ts");
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains(".mts")));
    }

    #[test]
    fn test_eslint_config_package() {
        let content = "export default {}";
        let result = ConfigValidationRuleSet::validate_eslint_config(content, "eslint.config.mts");
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains("@job-aide")));
    }

    #[test]
    fn test_tailwind_missing_content() {
        let content = r#"export default { theme: { extend: {} } }"#;
        let result =
            ConfigValidationRuleSet::validate_tailwind_config(content, "tailwind.config.ts");
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains("content")));
    }

    #[test]
    fn test_package_json_missing_type() {
        let content = r#"{ "name": "test" }"#;
        let result = ConfigValidationRuleSet::validate_package_json(content);
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains("type")));
    }

    #[test]
    fn test_package_json_npm_commands() {
        let content = r#"{ "scripts": { "build": "npm run tsc" } }"#;
        let result = ConfigValidationRuleSet::validate_package_json(content);
        assert!(result.is_ok());
        let violations = result.unwrap();
        assert!(violations.iter().any(|v| v.message.contains("npm")));
    }

    #[test]
    fn scan_flags_tsconfig_missing_strict() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{ "compilerOptions": {} }"#,
        )?;
        let issues = ConfigValidationScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "tsconfig-strict-mode" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn scan_flags_package_json_npm_scripts() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "type": "module", "scripts": { "build": "npm run tsc" } }"#,
        )?;
        let issues = ConfigValidationScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "package-json-no-npm-scripts" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn scan_silent_on_empty_project() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let issues = ConfigValidationScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_skips_node_modules() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let nm = dir.path().join("node_modules").join("pkg");
        std::fs::create_dir_all(&nm)?;
        std::fs::write(nm.join("tsconfig.json"), r#"{ "compilerOptions": {} }"#)?;
        let issues = ConfigValidationScanner::new().scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn scan_disables_eslint_base_when_configured() -> anyhow::Result<()> {
        let dir = tempfile::TempDir::new()?;
        std::fs::write(dir.path().join("eslint.config.mts"), "export default {}")?;
        let scanner = ConfigValidationScanner::with_config(None, true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "eslint-config-base"));
        Ok(())
    }
}
