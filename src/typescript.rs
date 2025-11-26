/// TypeScript-specific linting rules
/// Implements rules from job-aide typescript-rules.md

use crate::detection::{DetectionIssue, FunctionCallDetector, FunctionCallRule, PatternDetector, PatternRule};
use std::path::Path;
use tracing::debug;

pub struct TypeScriptRuleSet;

impl TypeScriptRuleSet {
    /// File extension rules - detect ambiguous extensions
    pub fn file_extension_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "ambiguous_ts_extension".to_string(),
                pattern: r"\.ts$".to_string(),
                severity: "high".to_string(),
                message_template: "❌ Ambiguous TypeScript extension '.ts' detected. Use '.mts' (ESM), '.cts' (CommonJS), or '.test.ts' for tests.".to_string(),
                fix_template: None,
                case_sensitive: true,
            },
            PatternRule {
                name: "ambiguous_js_extension".to_string(),
                pattern: r"\.js$".to_string(),
                severity: "high".to_string(),
                message_template: "❌ Ambiguous JavaScript extension '.js' detected. Use '.mjs' (ESM), '.cjs' (CommonJS), or '.config.js' for config.".to_string(),
                fix_template: None,
                case_sensitive: true,
            },
        ]
    }

    /// Path alias rules - detect ambiguous or conflicting aliases
    pub fn path_alias_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "ambiguous_path_alias".to_string(),
                pattern: r#""@/\*":\s*\["\.\/src\/\*"\]"#.to_string(),
                severity: "high".to_string(),
                message_template: "❌ Ambiguous path alias '@/*' detected. Use explicit category aliases like '@/core/*', '@/features/*', '@/components/*', '@/utils/*', '@/lib/*', '@/types/*'.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
            PatternRule {
                name: "conflicting_scoped_package".to_string(),
                pattern: r#""@/\*":\s*\["#.to_string(),
                severity: "medium".to_string(),
                message_template: "⚠️  Path alias '@/*' may conflict with npm scoped packages like '@radix-ui/*'. Use explicit category-based aliases instead.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
        ]
    }

    /// Module system rules - detect require() in ESM and import in CommonJS
    pub fn module_system_rules() -> Vec<FunctionCallRule> {
        vec![
            FunctionCallRule {
                name: "require_in_esm".to_string(),
                function_names: vec!["require".to_string()],
                severity: "high".to_string(),
                message_template: "❌ 'require()' detected at {file}:{line}. Use 'import' statements in ESM files (.mts, .tsx).".to_string(),
                fix_template: Some("import".to_string()),
            },
        ]
    }

    /// Code style rules - detect single quotes, improper indentation, missing semicolons
    pub fn code_style_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "single_quotes".to_string(),
                pattern: r"'[^']*'".to_string(),
                severity: "medium".to_string(),
                message_template: "⚠️  Single quotes detected: {matched}. Use double quotes (\") instead.".to_string(),
                fix_template: Some("\"replacement\"".to_string()),
                case_sensitive: true,
            },
            PatternRule {
                name: "missing_semicolon".to_string(),
                pattern: r"(const|let|var|function|return|import|export)\s+[^;]*[^;{}\s]$".to_string(),
                severity: "medium".to_string(),
                message_template: "⚠️  Missing semicolon at end of statement: {matched}".to_string(),
                fix_template: Some("{matched};".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "interface_instead_of_type".to_string(),
                pattern: r"\binterface\s+\w+".to_string(),
                severity: "low".to_string(),
                message_template: "ℹ️  'interface' detected: {matched}. Prefer 'type' for type definitions.".to_string(),
                fix_template: Some("type".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "import_without_type".to_string(),
                pattern: r"import\s+\{[^}]*\}\s+from".to_string(),
                severity: "low".to_string(),
                message_template: "ℹ️  Import statement detected: {matched}. Use 'import type' for type-only imports.".to_string(),
                fix_template: Some("import type".to_string()),
                case_sensitive: false,
            },
        ]
    }

    /// Package structure rules - detect missing documentation and tests
    pub fn package_structure_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "missing_readme".to_string(),
                pattern: r"README\.md".to_string(),
                severity: "info".to_string(),
                message_template: "ℹ️  README.md found. Ensure it contains usage and examples.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
            PatternRule {
                name: "missing_jsdoc".to_string(),
                pattern: r"export\s+(function|const|class|interface|type)\s+\w+".to_string(),
                severity: "low".to_string(),
                message_template: "ℹ️  Public API detected: {matched}. Add JSDoc comments.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
        ]
    }

    /// ESLint configuration rules
    pub fn eslint_config_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "missing_eslint_config".to_string(),
                pattern: r"eslint\.config\.(mts|ts|js)".to_string(),
                severity: "info".to_string(),
                message_template: "ℹ️  ESLint config file found: {matched}. Ensure it uses @job-aide/tools-lint-eslint-config.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
            PatternRule {
                name: "direct_process_env".to_string(),
                pattern: r"process\.env\.\w+".to_string(),
                severity: "medium".to_string(),
                message_template: "⚠️  Direct process.env access detected: {matched}. Use config abstraction instead.".to_string(),
                fix_template: Some("config.get()".to_string()),
                case_sensitive: true,
            },
        ]
    }

    /// Test file rules
    pub fn test_file_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "wrong_test_extension".to_string(),
                pattern: r"\.test\.ts$".to_string(),
                severity: "high".to_string(),
                message_template: "❌ Test file with ambiguous extension '.test.ts' detected. Use '.test.mts' for ESM test files.".to_string(),
                fix_template: Some(".test.mts".to_string()),
                case_sensitive: true,
            },
            PatternRule {
                name: "vitest_import".to_string(),
                pattern: r"(describe|it|test|expect)\s*\(".to_string(),
                severity: "info".to_string(),
                message_template: "ℹ️  Test framework detected: {matched}. Ensure Vitest is configured.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
        ]
    }
}

pub struct TypeScriptScanner {
    file_extension_detector: PatternDetector,
    path_alias_detector: PatternDetector,
    module_system_detector: FunctionCallDetector,
    code_style_detector: PatternDetector,
    package_structure_detector: PatternDetector,
    eslint_config_detector: PatternDetector,
    test_file_detector: PatternDetector,
}

impl TypeScriptScanner {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            file_extension_detector: PatternDetector::new(
                TypeScriptRuleSet::file_extension_rules(),
            )?,
            path_alias_detector: PatternDetector::new(
                TypeScriptRuleSet::path_alias_rules(),
            )?,
            module_system_detector: FunctionCallDetector::new(
                TypeScriptRuleSet::module_system_rules(),
            ),
            code_style_detector: PatternDetector::new(
                TypeScriptRuleSet::code_style_rules(),
            )?,
            package_structure_detector: PatternDetector::new(
                TypeScriptRuleSet::package_structure_rules(),
            )?,
            eslint_config_detector: PatternDetector::new(
                TypeScriptRuleSet::eslint_config_rules(),
            )?,
            test_file_detector: PatternDetector::new(
                TypeScriptRuleSet::test_file_rules(),
            )?,
        })
    }

    /// Scan a TypeScript/JavaScript file for violations
    pub fn scan_file(&self, file_path: &Path) -> Result<Vec<DetectionIssue>, Box<dyn std::error::Error>> {
        let mut all_issues = Vec::new();
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

        // Check file extensions
        all_issues.extend(self.file_extension_detector.scan_file(file_path)?);

        // Check for path aliases in tsconfig.json and package.json
        if file_name == "tsconfig.json" || file_name == "package.json" {
            all_issues.extend(self.path_alias_detector.scan_file(file_path)?);
            all_issues.extend(self.eslint_config_detector.scan_file(file_path)?);
        }

        // Check module system violations in TypeScript/JavaScript files
        let is_ts_file = file_name.ends_with(".ts")
            || file_name.ends_with(".mts")
            || file_name.ends_with(".cts")
            || file_name.ends_with(".tsx")
            || file_name.ends_with(".js")
            || file_name.ends_with(".mjs")
            || file_name.ends_with(".cjs")
            || file_name.ends_with(".jsx");

        if is_ts_file {
            all_issues.extend(self.module_system_detector.scan_file(file_path)?);
            all_issues.extend(self.code_style_detector.scan_file(file_path)?);
            all_issues.extend(self.package_structure_detector.scan_file(file_path)?);
        }

        // Check test files
        if file_name.ends_with(".test.ts")
            || file_name.ends_with(".test.mts")
            || file_name.ends_with(".test.js")
            || file_name.ends_with(".spec.ts")
            || file_name.ends_with(".spec.mts")
        {
            all_issues.extend(self.test_file_detector.scan_file(file_path)?);
        }

        Ok(all_issues)
    }

    /// Apply fixes to a file
    pub fn apply_fixes(
        &self,
        file_path: &Path,
        issues: &[DetectionIssue],
        dry_run: bool,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let (_, fixes_applied) = self.code_style_detector.apply_fixes(file_path, issues, dry_run)?;
        Ok(fixes_applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_rules_created() {
        let ext_rules = TypeScriptRuleSet::file_extension_rules();
        assert!(!ext_rules.is_empty());

        let alias_rules = TypeScriptRuleSet::path_alias_rules();
        assert!(!alias_rules.is_empty());

        let module_rules = TypeScriptRuleSet::module_system_rules();
        assert!(!module_rules.is_empty());

        let style_rules = TypeScriptRuleSet::code_style_rules();
        assert!(!style_rules.is_empty());

        let pkg_rules = TypeScriptRuleSet::package_structure_rules();
        assert!(!pkg_rules.is_empty());

        let eslint_rules = TypeScriptRuleSet::eslint_config_rules();
        assert!(!eslint_rules.is_empty());

        let test_rules = TypeScriptRuleSet::test_file_rules();
        assert!(!test_rules.is_empty());
    }

    #[test]
    fn test_scanner_creation() {
        let scanner = TypeScriptScanner::new();
        assert!(scanner.is_ok());
    }
}
