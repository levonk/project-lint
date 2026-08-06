/// Generic pattern detection and replacement module
/// Provides reusable functionality for string/regex-based detection and auto-fixing
use regex::Regex;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct DetectionIssue {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub pattern_name: String,
    pub matched_text: String,
    pub message: String,
    pub severity: String,
    pub fix: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PatternRule {
    pub name: String,
    pub pattern: String,
    pub severity: String,
    pub message_template: String,
    pub fix_template: Option<String>,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone)]
pub struct FunctionCallRule {
    pub name: String,
    pub function_names: Vec<String>,
    pub severity: String,
    pub message_template: String,
    pub fix_template: Option<String>,
}

pub struct PatternDetector {
    patterns: Vec<(PatternRule, Regex)>,
}

impl PatternDetector {
    /// Compile a set of [`PatternRule`]s into a detector.
    ///
    /// Each rule's `pattern` is wrapped with case-sensitivity flags and
    /// compiled once at construction; [`PatternDetector::scan_str`] reuses the
    /// compiled regexes for every scan.
    ///
    /// ```rust
    /// use project_lint_core::scanners::detection::{PatternDetector, PatternRule};
    ///
    /// let detector = PatternDetector::new(vec![PatternRule {
    ///     name: "todo".to_string(),
    ///     pattern: r"TODO\(\w+\)".to_string(),
    ///     severity: "warning".to_string(),
    ///     message_template: "{matched}".to_string(),
    ///     fix_template: None,
    ///     case_sensitive: true,
    /// }]).expect("regex compiles");
    ///
    /// let issues = detector.scan_str("let x = TODO(alice);", "mem");
    /// assert_eq!(issues.len(), 1);
    /// assert_eq!(issues[0].matched_text, "TODO(alice)");
    /// ```
    pub fn new(rules: Vec<PatternRule>) -> Result<Self, regex::Error> {
        let mut patterns = Vec::new();
        for rule in rules {
            let regex_flags = if rule.case_sensitive {
                format!("(?-i){}", rule.pattern)
            } else {
                format!("(?i){}", rule.pattern)
            };
            let compiled = Regex::new(&regex_flags)?;
            patterns.push((rule, compiled));
        }
        Ok(Self { patterns })
    }

    /// Scan a file for pattern matches
    pub fn scan_file(&self, file_path: &Path) -> Result<Vec<DetectionIssue>, std::io::Error> {
        let content = fs::read_to_string(file_path)?;
        Ok(self.scan_str(&content, file_path.to_string_lossy().as_ref()))
    }

    /// Scan an in-memory string for pattern matches. The `file_label` is used
    /// only for issue attribution (no disk IO). Useful for property tests and
    /// callers that already have content in memory.
    pub fn scan_str(&self, content: &str, file_label: &str) -> Vec<DetectionIssue> {
        let mut issues = Vec::new();

        for (rule, regex) in &self.patterns {
            for (line_num, line) in content.lines().enumerate() {
                for cap in regex.captures_iter(line) {
                    let matched_text = cap.get(0).unwrap().as_str().to_string();
                    let column = cap.get(0).unwrap().start();

                    let message = rule
                        .message_template
                        .replace("{matched}", &matched_text)
                        .replace("{file}", file_label)
                        .replace("{line}", &(line_num + 1).to_string())
                        .replace("{column}", &column.to_string());

                    let fix = rule.fix_template.as_ref().map(|template| {
                        template
                            .replace("{matched}", &matched_text)
                            .replace("{file}", file_label)
                    });

                    issues.push(DetectionIssue {
                        file: file_label.to_string(),
                        line: line_num + 1,
                        column,
                        pattern_name: rule.name.clone(),
                        matched_text: matched_text.clone(),
                        message,
                        severity: rule.severity.clone(),
                        fix,
                    });

                    debug!(
                        "Pattern '{}' matched in {}: {}",
                        rule.name, file_label, matched_text
                    );
                }
            }
        }

        issues
    }

    /// Apply fixes to a file (returns modified content)
    pub fn apply_fixes(
        &self,
        file_path: &Path,
        issues: &[DetectionIssue],
        dry_run: bool,
    ) -> Result<(String, usize), std::io::Error> {
        let mut content = fs::read_to_string(file_path)?;
        let mut fixes_applied = 0;

        // Sort issues by line in reverse to avoid offset issues
        let mut sorted_issues = issues.to_vec();
        sorted_issues.sort_by(|a, b| b.line.cmp(&a.line));

        for issue in sorted_issues {
            if let Some(fix) = &issue.fix {
                let lines: Vec<&str> = content.lines().collect();
                if issue.line > 0 && issue.line <= lines.len() {
                    let line = lines[issue.line - 1];
                    let fixed_line = line.replace(&issue.matched_text, fix);

                    // Reconstruct content
                    let mut new_lines = lines.clone();
                    new_lines[issue.line - 1] = &fixed_line;
                    content = new_lines.join("\n");
                    fixes_applied += 1;

                    debug!(
                        "Fixed '{}' in {} at line {}",
                        issue.pattern_name,
                        file_path.display(),
                        issue.line
                    );
                }
            }
        }

        if !dry_run && fixes_applied > 0 {
            fs::write(file_path, &content)?;
        }

        Ok((content, fixes_applied))
    }
}

pub struct FunctionCallDetector {
    rules: Vec<FunctionCallRule>,
}

impl FunctionCallDetector {
    pub fn new(rules: Vec<FunctionCallRule>) -> Self {
        Self { rules }
    }

    /// Scan for function calls
    pub fn scan_file(&self, file_path: &Path) -> Result<Vec<DetectionIssue>, std::io::Error> {
        let content = fs::read_to_string(file_path)?;
        let mut issues = Vec::new();

        for rule in &self.rules {
            for (line_num, line) in content.lines().enumerate() {
                for func_name in &rule.function_names {
                    // Match function calls: func_name followed by (
                    let pattern = format!(r"\b{}\s*\(", regex::escape(func_name));
                    if let Ok(regex) = Regex::new(&pattern) {
                        for cap in regex.captures_iter(line) {
                            let matched_text = cap.get(0).unwrap().as_str().to_string();
                            let column = cap.get(0).unwrap().start();

                            let message = rule
                                .message_template
                                .replace("{function}", func_name)
                                .replace("{file}", file_path.to_string_lossy().as_ref())
                                .replace("{line}", &(line_num + 1).to_string())
                                .replace("{column}", &column.to_string());

                            let fix = rule.fix_template.as_ref().map(|template| {
                                template
                                    .replace("{function}", func_name)
                                    .replace("{file}", file_path.to_string_lossy().as_ref())
                            });

                            issues.push(DetectionIssue {
                                file: file_path.to_string_lossy().to_string(),
                                line: line_num + 1,
                                column,
                                pattern_name: rule.name.clone(),
                                matched_text: matched_text.clone(),
                                message,
                                severity: rule.severity.clone(),
                                fix,
                            });

                            debug!(
                                "Function call '{}' found in {}: {}",
                                rule.name,
                                file_path.display(),
                                matched_text
                            );
                        }
                    }
                }
            }
        }

        Ok(issues)
    }

    /// Apply fixes to a file
    pub fn apply_fixes(
        &self,
        file_path: &Path,
        issues: &[DetectionIssue],
        dry_run: bool,
    ) -> Result<(String, usize), std::io::Error> {
        let mut content = fs::read_to_string(file_path)?;
        let mut fixes_applied = 0;

        // Sort issues by line in reverse to avoid offset issues
        let mut sorted_issues = issues.to_vec();
        sorted_issues.sort_by(|a, b| b.line.cmp(&a.line));

        for issue in sorted_issues {
            if let Some(fix) = &issue.fix {
                let lines: Vec<&str> = content.lines().collect();
                if issue.line > 0 && issue.line <= lines.len() {
                    let line = lines[issue.line - 1];
                    let fixed_line = line.replace(&issue.matched_text, fix);

                    // Reconstruct content
                    let mut new_lines = lines.clone();
                    new_lines[issue.line - 1] = &fixed_line;
                    content = new_lines.join("\n");
                    fixes_applied += 1;

                    debug!(
                        "Fixed '{}' in {} at line {}",
                        issue.pattern_name,
                        file_path.display(),
                        issue.line
                    );
                }
            }
        }

        if !dry_run && fixes_applied > 0 {
            fs::write(file_path, &content)?;
        }

        Ok((content, fixes_applied))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prop_assert;

    #[test]
    fn test_pattern_detection() {
        let rules = vec![PatternRule {
            name: "test_pattern".to_string(),
            pattern: r#"password\s*=\s*['"].*['"]"#.to_string(),
            severity: "critical".to_string(),
            message_template: "Found hardcoded password: {matched}".to_string(),
            fix_template: Some("password = os.getenv('PASSWORD')".to_string()),
            case_sensitive: false,
        }];

        let detector = PatternDetector::new(rules).unwrap();
        assert!(!detector.patterns.is_empty());
    }

    #[test]
    fn test_function_call_detection() {
        let rules = vec![FunctionCallRule {
            name: "unsafe_strcpy".to_string(),
            function_names: vec!["strcpy".to_string()],
            severity: "high".to_string(),
            message_template: "Unsafe function '{function}' at {file}:{line}".to_string(),
            fix_template: Some("snprintf".to_string()),
        }];

        let detector = FunctionCallDetector::new(rules);
        assert_eq!(detector.rules.len(), 1);
    }

    #[test]
    fn test_pattern_detector_apply_fixes_replaces_matched_text() {
        use tempfile::NamedTempFile;
        let rules = vec![PatternRule {
            name: "todo".to_string(),
            pattern: r"TODO\(\w+\)".to_string(),
            severity: "warning".to_string(),
            message_template: "Found TODO: {matched}".to_string(),
            fix_template: Some("DONE".to_string()),
            case_sensitive: true,
        }];
        let detector = PatternDetector::new(rules).unwrap();

        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "let x = TODO(alice);\n").unwrap();
        let issues = detector.scan_file(file.path()).unwrap();
        assert_eq!(issues.len(), 1);

        // dry_run: no write, returns fixed content
        let (content, n) = detector.apply_fixes(file.path(), &issues, true).unwrap();
        assert_eq!(n, 1);
        assert!(content.contains("DONE"));
        // file on disk unchanged
        assert!(std::fs::read_to_string(file.path())
            .unwrap()
            .contains("TODO(alice)"));

        // real fix: file on disk updated
        let (_, n) = detector.apply_fixes(file.path(), &issues, false).unwrap();
        assert_eq!(n, 1);
        let on_disk = std::fs::read_to_string(file.path()).unwrap();
        assert!(on_disk.contains("DONE"));
        assert!(!on_disk.contains("TODO(alice)"));
    }

    #[test]
    fn test_pattern_detector_apply_fixes_no_fix_template_is_noop() {
        use tempfile::NamedTempFile;
        let rules = vec![PatternRule {
            name: "marker".to_string(),
            pattern: r"MARKER".to_string(),
            severity: "info".to_string(),
            message_template: "marker: {matched}".to_string(),
            fix_template: None,
            case_sensitive: true,
        }];
        let detector = PatternDetector::new(rules).unwrap();
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "MARKER here\n").unwrap();
        let issues = detector.scan_file(file.path()).unwrap();
        let (_, n) = detector.apply_fixes(file.path(), &issues, false).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_scan_file_missing_path_returns_io_error() {
        let rules = vec![PatternRule {
            name: "x".to_string(),
            pattern: "x".to_string(),
            severity: "info".to_string(),
            message_template: "x".to_string(),
            fix_template: None,
            case_sensitive: true,
        }];
        let detector = PatternDetector::new(rules).unwrap();
        let result = detector.scan_file(std::path::Path::new("/nonexistent/does/not/exist.txt"));
        assert!(result.is_err(), "scanning a missing file should error");
    }

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 64, ..proptest::test_runner::Config::default()
        })]

        #[test]
        fn regex_always_matches_substring(
            ref needle in "[a-z]{1,5}",
            ref suffix in "[a-z]{0,5}"
        ) {
            // A literal needle regex must always match a line containing it,
            // regardless of trailing text. Case-insensitive wrapping is applied
            // by PatternDetector, so uppercase needles also match lowercase.
            let rules = vec![PatternRule {
                name: "needle".to_string(),
                pattern: regex::escape(needle),
                severity: "info".to_string(),
                message_template: "n".to_string(),
                fix_template: None,
                case_sensitive: false,
            }];
            let detector = PatternDetector::new(rules).expect("regex");
            let line = format!("{}{}", needle, suffix);
            let issues = detector.scan_str(&line, "mem");
            prop_assert!(!issues.is_empty(), "needle {:?} not found in {:?}", needle, line);
        }

        #[test]
        fn regex_no_match_when_disjoint(ref hay in "[a-z]{0,10}") {
            let rules = vec![PatternRule {
                name: "digit".to_string(),
                pattern: "[0-9]".to_string(),
                severity: "info".to_string(),
                message_template: "d".to_string(),
                fix_template: None,
                case_sensitive: true,
            }];
            let detector = PatternDetector::new(rules).expect("regex");
            let issues = detector.scan_str(hay, "mem");
            prop_assert!(issues.is_empty(), "digit matched in pure-alpha {:?}", hay);
        }
    }
}
