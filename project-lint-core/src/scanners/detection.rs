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
        let mut issues = Vec::new();

        for (rule, regex) in &self.patterns {
            for (line_num, line) in content.lines().enumerate() {
                for cap in regex.captures_iter(line) {
                    let matched_text = cap.get(0).unwrap().as_str().to_string();
                    let column = cap.get(0).unwrap().start();

                    let message = rule
                        .message_template
                        .replace("{matched}", &matched_text)
                        .replace("{file}", file_path.to_string_lossy().as_ref())
                        .replace("{line}", &(line_num + 1).to_string())
                        .replace("{column}", &column.to_string());

                    let fix = rule.fix_template.as_ref().map(|template| {
                        template
                            .replace("{matched}", &matched_text)
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
                        "Pattern '{}' matched in {}: {}",
                        rule.name,
                        file_path.display(),
                        matched_text
                    );
                }
            }
        }

        Ok(issues)
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
}
