/// Configuration file validation rules
/// Validates tsconfig.json, eslint.config.mts, tailwind.config.ts, package.json

use regex::Regex;
use std::path::Path;
use tracing::debug;

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
                message: "moduleResolution not configured. Recommended: \"bundler\" or \"node\"".to_string(),
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
    pub fn validate_eslint_config(content: &str, file_name: &str) -> Result<Vec<ConfigViolation>, String> {
        let mut violations = Vec::new();

        // Check file extension
        if file_name != "eslint.config.mts" {
            violations.push(ConfigViolation {
                file: file_name.to_string(),
                severity: "high".to_string(),
                message: "ESLint config must be named eslint.config.mts (not .ts or .js)".to_string(),
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
                message: "Web project should include runtime guards plugin for browser safety".to_string(),
            });
        }

        Ok(violations)
    }

    /// Validate tailwind.config.ts
    pub fn validate_tailwind_config(content: &str, file_name: &str) -> Result<Vec<ConfigViolation>, String> {
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
                message: "Tailwind content configuration missing. Add content array with file patterns".to_string(),
            });
        }

        // Check if content is empty
        if content.contains("content: []") || content.contains("content: [ ]") {
            violations.push(ConfigViolation {
                file: file_name.to_string(),
                severity: "high".to_string(),
                message: "Tailwind content array is empty. Add file patterns for purging".to_string(),
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
                message: "Missing \"type\" field. Add \"type\": \"module\" for ESM packages".to_string(),
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
        let result = ConfigValidationRuleSet::validate_tailwind_config(content, "tailwind.config.ts");
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
}
