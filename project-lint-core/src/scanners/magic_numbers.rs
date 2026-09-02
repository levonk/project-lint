//! Magic-number scanner — detects hardcoded IPs, ports, and magic numbers
//! that should be named variables.
//!
//! Rationale: infrastructure-as-code repos (Ansible, Docker Compose, etc.)
//! frequently hardcode IP addresses, port numbers, UIDs, modes, and retry
//! counts in task/template files. These values should be defined as variables
//! in designated definition directories (`defaults/`, `vars/`, `group_vars/`,
//! `host_vars/`, `infrastructure/`) and referenced via `{{ var }}` elsewhere.
//!
//! ## Rule summary
//!
//! A "magic literal" is any of:
//! - IPv4 address           (e.g. `100.90.22.85`, `127.0.0.1`, `0.0.0.0`)
//! - IPv4 CIDR              (e.g. `172.26.0.0/16`)
//! - IPv4:port              (e.g. `100.90.22.85:5000`)
//! - Dotted-decimal version (e.g. `0.9.25`, `1.2.3`)
//! - Bare integer           (e.g. `30`, `1000`, `755`, `100000`)
//! - Integer + unit suffix  (e.g. `10m`, `30s`, `5M`, `1h`, `200ms`)
//!
//! A magic literal is a **violation** unless ALL of:
//! 1. The file is inside an allowlisted definition directory, AND
//! 2. The line is a direct variable assignment (`key: value`), AND
//! 3. The literal is NOT inside a `{{ ... }}` Jinja2 expression.
//!
//! In non-allowlisted directories (tasks/, playbooks/, templates/, etc.)
//! ANY magic literal is a violation — it must be a `{{ var }}` reference.
//!
//! ## Inline overrides
//!
//! Suppress specific rules on a line with:
//! ```text
//! # project-lint: disable=magic-ipv4,magic-ipport
//! ```
//! Rule names use the `magic-` prefix (matching the scanner's rule output):
//! `magic-cidr`, `magic-ipport`, `magic-ipv4`, `magic-dotted`,
//! `magic-unitnum`, `magic-int`.
//! Use `disable=all` or bare `disable` to suppress all rules on the line.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use regex::Regex;
use std::path::Path;

/// Configuration for the magic-number scanner.
#[derive(Debug, Clone, Default)]
pub struct MagicNumbersConfig {
    /// Directory names where variable definitions are expected.
    /// A `key: <literal>` line here is exempt; a literal inside `{{ }}` is not.
    /// Defaults to: `defaults`, `vars`, `group_vars`, `host_vars`, `infrastructure`.
    pub definition_dirs: Vec<String>,

    /// Directory names that are fully exempt from scanning (docs, tests, etc.).
    /// Defaults to: `internal-docs`, `.agents`, `.claude`, `.devin`, `.git`,
    /// `.molecule`, `collections`, `node_modules`, `.venv`, `.cache`,
    /// `08-docs`, `docs`, `tests`, `test`, `__pycache__`.
    pub exempt_dirs: Vec<String>,

    /// File extensions to scan. Defaults to `.yml`, `.yaml`, `.j2`, `.jinja2`.
    pub scan_extensions: Vec<String>,

    /// Filename substrings that mark generated/lock files (always exempt).
    /// Defaults to: `lock`, `.vault`.
    pub exempt_name_substrings: Vec<String>,

    /// When true, flag every literal everywhere (even in definition dirs).
    pub strict: bool,

    /// When true, ignore inline `# project-lint: disable=...` overrides.
    pub ignore_overrides: bool,
}

impl MagicNumbersConfig {
    /// Create a config with sensible defaults for Ansible/IaC repos.
    pub fn default_for_iac() -> Self {
        Self {
            definition_dirs: vec![
                "defaults".into(),
                "vars".into(),
                "group_vars".into(),
                "host_vars".into(),
                "infrastructure".into(),
            ],
            exempt_dirs: vec![
                "internal-docs".into(),
                ".agents".into(),
                ".claude".into(),
                ".devin".into(),
                ".git".into(),
                ".molecule".into(),
                "collections".into(),
                "node_modules".into(),
                ".venv".into(),
                ".cache".into(),
                "08-docs".into(),
                "docs".into(),
                "tests".into(),
                "test".into(),
                "__pycache__".into(),
            ],
            scan_extensions: vec![
                ".yml".into(),
                ".yaml".into(),
                ".j2".into(),
                ".jinja2".into(),
            ],
            exempt_name_substrings: vec!["lock".into(), ".vault".into()],
            strict: false,
            ignore_overrides: false,
        }
    }
}

/// The kind of magic literal detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagicKind {
    Cidr,
    IpPort,
    Ipv4,
    Dotted,
    UnitNum,
    Int,
}

impl MagicKind {
    fn as_str(&self) -> &'static str {
        match self {
            MagicKind::Cidr => "cidr",
            MagicKind::IpPort => "ipport",
            MagicKind::Ipv4 => "ipv4",
            MagicKind::Dotted => "dotted",
            MagicKind::UnitNum => "unitnum",
            MagicKind::Int => "int",
        }
    }
}

/// A single magic-literal violation.
#[derive(Debug, Clone)]
struct Violation {
    file: String,
    line: usize,
    column: usize,
    token: String,
    kind: MagicKind,
}

/// Compiled regex patterns for numeric literal detection.
/// The Rust `regex` crate does not support look-around, so boundary checks
/// are done in Rust code after matching.
struct NumericPatterns {
    cidr: Regex,
    ipport: Regex,
    ipv4: Regex,
    dotted: Regex,
    unitnum: Regex,
    int: Regex,
}

impl NumericPatterns {
    fn new() -> Self {
        Self {
            // Match broadly, filter with boundary checks in find_all().
            cidr: Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}/\d{1,2}\b").unwrap(),
            ipport: Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}:\d{1,5}\b").unwrap(),
            ipv4: Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            dotted: Regex::new(r"\d+\.\d+(?:\.\d+)*").unwrap(),
            unitnum: Regex::new(r"\d+(?:ms|s|m|h|k|M|G|T|B)\b").unwrap(),
            int: Regex::new(r"\d+").unwrap(),
        }
    }

    /// Check that the character before `pos` is not a word char, dot, or `[`.
    /// This replaces the Python look-behind `(?<![\w.\[])`.
    fn has_clean_prefix(line: &str, pos: usize) -> bool {
        if pos == 0 {
            return true;
        }
        let bytes = line.as_bytes();
        let prev = bytes[pos - 1];
        // Not a word char, not a dot, not `[`
        !prev.is_ascii_alphanumeric() && prev != b'_' && prev != b'.' && prev != b'['
    }

    /// Check that the character before `pos` is not a letter preceded by `/`.
    /// This replaces the Python look-behind `(?<![A-Za-z]/)`.
    /// Catches `HTTP/1.1`, `HTTP/2.0` where the number is preceded by `/`.
    fn has_no_slash_letter_prefix(line: &str, pos: usize) -> bool {
        if pos < 2 {
            return true;
        }
        let bytes = line.as_bytes();
        // Check for pattern: letter, /, then our match
        !(bytes[pos - 1] == b'/' && bytes[pos - 2].is_ascii_alphabetic())
    }

    /// Check that the character before `pos` is not a letter-hyphen combo.
    /// This replaces the Python look-behind `(?<![A-Za-z]\-)`.
    /// Catches `02-config` where `02` is preceded by nothing but the
    /// hyphen after it makes it a path component.
    fn has_no_letter_hyphen_before(line: &str, pos: usize) -> bool {
        if pos < 2 {
            return true;
        }
        let bytes = line.as_bytes();
        !(bytes[pos - 1] == b'-' && bytes[pos - 2].is_ascii_alphabetic())
    }

    /// Check that the character after `end` is not a word char, dot, `]`, or `>`.
    /// This replaces the Python look-ahead `(?![\w.\]>])`.
    fn has_clean_suffix(line: &str, end: usize) -> bool {
        if end >= line.len() {
            return true;
        }
        let next = line.as_bytes()[end];
        !next.is_ascii_alphanumeric()
            && next != b'_'
            && next != b'.'
            && next != b']'
            && next != b'>'
    }

    /// Check that the characters after `end` are not hyphen-letter.
    /// This replaces the Python look-ahead `(?!\-[A-Za-z])`.
    fn has_no_hyphen_letter_after(line: &str, end: usize) -> bool {
        let bytes = line.as_bytes();
        if end + 1 >= bytes.len() {
            return true;
        }
        !(bytes[end] == b'-' && bytes[end + 1].is_ascii_alphabetic())
    }

    /// Check that the character after `end` is not a dot (for dotted version
    /// suffix — prevents matching `65` in `v0.65.0` when the full `65.0` should
    /// match instead).
    fn has_no_dot_after(line: &str, end: usize) -> bool {
        if end >= line.len() {
            return true;
        }
        line.as_bytes()[end] != b'.'
    }

    /// Find all numeric matches in a line, preserving priority order.
    /// Returns (start, kind, token) tuples sorted by position.
    fn find_all<'a>(&self, line: &'a str) -> Vec<(usize, MagicKind, &'a str)> {
        let mut matches: Vec<(usize, usize, MagicKind, &'a str)> = Vec::new();

        // CIDR: check clean prefix and no slash-letter before
        for m in self.cidr.find_iter(line) {
            if Self::has_clean_prefix(line, m.start())
                && Self::has_no_slash_letter_prefix(line, m.start())
            {
                matches.push((m.start(), m.end(), MagicKind::Cidr, m.as_str()));
            }
        }
        // IP:port: same prefix checks
        for m in self.ipport.find_iter(line) {
            if Self::has_clean_prefix(line, m.start())
                && Self::has_no_slash_letter_prefix(line, m.start())
            {
                matches.push((m.start(), m.end(), MagicKind::IpPort, m.as_str()));
            }
        }
        // IPv4: same prefix checks
        for m in self.ipv4.find_iter(line) {
            if Self::has_clean_prefix(line, m.start())
                && Self::has_no_slash_letter_prefix(line, m.start())
            {
                matches.push((m.start(), m.end(), MagicKind::Ipv4, m.as_str()));
            }
        }
        // Dotted: prefix checks + no dot after (so 65.0 matches, not 65 from 65.0.x)
        for m in self.dotted.find_iter(line) {
            if Self::has_clean_prefix(line, m.start())
                && Self::has_no_slash_letter_prefix(line, m.start())
                && Self::has_no_dot_after(line, m.end())
            {
                matches.push((m.start(), m.end(), MagicKind::Dotted, m.as_str()));
            }
        }
        // Unit number: check clean prefix (not preceded by word char or dot)
        for m in self.unitnum.find_iter(line) {
            if Self::has_clean_prefix(line, m.start()) {
                matches.push((m.start(), m.end(), MagicKind::UnitNum, m.as_str()));
            }
        }
        // Int: full boundary checks (prefix + suffix + no hyphen-letter)
        for m in self.int.find_iter(line) {
            if Self::has_clean_prefix(line, m.start())
                && Self::has_no_letter_hyphen_before(line, m.start())
                && Self::has_clean_suffix(line, m.end())
                && Self::has_no_hyphen_letter_after(line, m.end())
            {
                matches.push((m.start(), m.end(), MagicKind::Int, m.as_str()));
            }
        }

        // Sort by start position.
        matches.sort_by_key(|(start, _, _, _)| *start);

        // Remove overlapping matches (earlier match wins).
        let mut result: Vec<(usize, MagicKind, &'a str)> = Vec::new();
        let mut last_end = 0;
        for (start, end, kind, token) in &matches {
            if *start >= last_end {
                result.push((*start, *kind, token));
                last_end = *end;
            }
        }
        result
    }
}

/// Compiled regex patterns for language-syntax stripping.
struct SyntaxPatterns {
    regex_quantifier: Regex,
    yaml_block_scalar: Regex,
    sed_backref: Regex,
    exit_code: Regex,
    cmp_zero_one: Regex,
    shell_redirect: Regex,
    shell_for: Regex,
    shell_seq: Regex,
    jinja: Regex,
    definition_line: Regex,
    override_re: Regex,
}

impl SyntaxPatterns {
    fn new() -> Self {
        Self {
            regex_quantifier: Regex::new(r"\{\d+(?:,\d*)?\}").unwrap(),
            // YAML block scalar: `|2` or `|-2` after `: `.  Can't use
            // look-behind in Rust regex, so match `: |2` and replace the
            // number only.
            yaml_block_scalar: Regex::new(r": \|[-+]?\d").unwrap(),
            sed_backref: Regex::new(r"\\\d+").unwrap(),
            exit_code: Regex::new(r"\bexit\s+\d+").unwrap(),
            cmp_zero_one: Regex::new(r"(?:==|!=|>=|<=|>|<|-eq|-ne|-gt|-lt|-ge|-le)\s*[01]\b")
                .unwrap(),
            shell_redirect: Regex::new(r"\d+>&\d+|\d+>\d+").unwrap(),
            shell_for: Regex::new(r"\bfor\s+\w+\s+in\s+[\d\s]+;").unwrap(),
            shell_seq: Regex::new(r"\bseq\s+\d+(?:\s+\d+)*").unwrap(),
            jinja: Regex::new(r"\{\{.*?\}\}").unwrap(),
            definition_line: Regex::new(r"^\s*[A-Za-z_][\w.-]*\s*:\s+\S").unwrap(),
            // # project-lint: disable=magic-int,magic-unitnum  OR  bare disable
            override_re: Regex::new(r"#\s*project-lint\s*:\s*disable\s*(?:=\s*([A-Za-z,\-]+))?")
                .unwrap(),
        }
    }

    /// Strip language-syntax patterns that contain numbers but are not data.
    fn strip_syntax(&self, code: &mut String) {
        *code = self.regex_quantifier.replace_all(code, "").to_string();
        // YAML block scalar: replace `: |2` with `: |` (strip the number).
        *code = self.yaml_block_scalar.replace_all(code, ": |").to_string();
        *code = self.sed_backref.replace_all(code, "").to_string();
        *code = self.exit_code.replace_all(code, "exit").to_string();
        *code = self.cmp_zero_one.replace_all(code, "CMP").to_string();
        *code = self.shell_redirect.replace_all(code, "").to_string();
        *code = self.shell_for.replace_all(code, "").to_string();
        *code = self.shell_seq.replace_all(code, "").to_string();
    }

    /// Find Jinja2 `{{ ... }}` spans in the line.
    fn jinja_spans(&self, code: &str) -> Vec<(usize, usize)> {
        self.jinja
            .find_iter(code)
            .map(|m| (m.start(), m.end()))
            .collect()
    }

    /// Check if a position is inside a Jinja2 expression.
    fn is_inside_jinja(&self, pos: usize, spans: &[(usize, usize)]) -> bool {
        spans.iter().any(|(start, end)| *start <= pos && pos < *end)
    }

    /// Check if a line is a YAML `key: value` mapping assignment.
    fn is_definition_line(&self, code: &str) -> bool {
        self.definition_line.is_match(code)
    }

    /// Parse an inline override from a line.
    /// Returns `None` if no override, or a set of rule names to suppress.
    fn parse_override(&self, line: &str) -> Option<Vec<String>> {
        let caps = self.override_re.captures(line)?;
        match caps.get(1) {
            None => Some(vec!["all".to_string()]),
            Some(rules_str) => {
                let rules: Vec<String> = rules_str
                    .as_str()
                    .split(',')
                    .map(|r| r.trim().to_lowercase())
                    .filter(|r| !r.is_empty())
                    .collect();
                Some(rules)
            }
        }
    }
}

/// Strip a trailing YAML comment, respecting quotes.
fn strip_comment(line: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = line.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '#' && !in_single && !in_double {
            if i == 0 || chars[i - 1] == ' ' || chars[i - 1] == '\t' {
                return line[..i].to_string();
            }
        }
    }
    line.to_string()
}

/// Check if a rule kind is suppressed by the override set.
/// Rule names in overrides use the `magic-` prefix (e.g. `magic-int`).
fn is_suppressed(kind: MagicKind, override_rules: &Option<Vec<String>>) -> bool {
    match override_rules {
        None => false,
        Some(rules) => {
            let rule_name = format!("magic-{}", kind.as_str());
            rules.iter().any(|r| r == "all" || r == &rule_name)
        }
    }
}

/// The magic-number scanner.
pub struct MagicNumbersScanner {
    config: MagicNumbersConfig,
    numeric: NumericPatterns,
    syntax: SyntaxPatterns,
    excluded: Vec<String>,
}

impl MagicNumbersScanner {
    /// Create a scanner with default IaC configuration.
    pub fn new() -> Self {
        Self::with_config(MagicNumbersConfig::default_for_iac())
    }

    /// Create a scanner with custom configuration.
    pub fn with_config(config: MagicNumbersConfig) -> Self {
        Self {
            config,
            numeric: NumericPatterns::new(),
            syntax: SyntaxPatterns::new(),
            excluded: build_exclusions(&[], false),
        }
    }

    /// Create a scanner with custom configuration and a centralized exclusion
    /// list (from `[scanner_config.exclusion]`).
    pub fn with_config_and_exclusions(config: MagicNumbersConfig, excluded: Vec<String>) -> Self {
        Self {
            config,
            numeric: NumericPatterns::new(),
            syntax: SyntaxPatterns::new(),
            excluded,
        }
    }

    /// Scan a project root for magic-number violations.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in
            walk_project(root, &self.excluded, usize::MAX).filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }

            if self.is_exempt(&rel) {
                continue;
            }

            // Check file extension.
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let full_ext = format!(".{}", ext);
            if !self.config.scan_extensions.iter().any(|e| e == &full_ext) {
                continue;
            }

            issues.extend(self.scan_file(path, &rel));
        }

        Ok(issues)
    }

    /// Scan a single file.
    pub fn scan_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };

        let is_def_dir = self.is_definition_dir(rel);
        let mut issues = Vec::new();

        for (line_no, line) in content.lines().enumerate() {
            let violations = self.scan_line(line, line_no + 1, rel, is_def_dir);
            for v in violations {
                issues.push(
                    ScannerIssue::new(
                        &format!("magic-{}", v.kind.as_str()),
                        "warning",
                        &v.file,
                        format!(
                            "magic {} '{}' should be a named variable",
                            v.kind.as_str(),
                            v.token
                        ),
                    )
                    .at_line(v.line),
                );
            }
        }

        issues
    }

    /// Scan a single line for magic-literal violations.
    fn scan_line(
        &self,
        line: &str,
        line_no: usize,
        rel: &str,
        is_definition_dir: bool,
    ) -> Vec<Violation> {
        // Parse inline override (unless --ignore-overrides is set).
        let override_rules = if self.config.ignore_overrides {
            None
        } else {
            self.syntax.parse_override(line)
        };

        let mut code = strip_comment(line);
        self.syntax.strip_syntax(&mut code);

        let spans = self.syntax.jinja_spans(&code);
        let mut violations = Vec::new();

        for (pos, kind, token) in self.numeric.find_all(&code) {
            let inside_jinja = self.syntax.is_inside_jinja(pos, &spans);

            let is_exempt = if self.config.strict {
                false
            } else if is_definition_dir && self.syntax.is_definition_line(&code) && !inside_jinja {
                true
            } else {
                false
            };

            if is_exempt {
                continue;
            }

            if is_suppressed(kind, &override_rules) {
                continue;
            }

            violations.push(Violation {
                file: rel.to_string(),
                line: line_no,
                column: pos + 1,
                token: token.to_string(),
                kind,
            });
        }

        violations
    }

    /// Check if a relative path is fully exempt from scanning.
    fn is_exempt(&self, rel: &str) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();

        // Exempt directory components anywhere in the path.
        for part in &parts {
            if self.config.exempt_dirs.iter().any(|d| d == part) {
                return true;
            }
        }

        // Generated / lock / vault files by name substring.
        let name_lower = parts.last().unwrap_or(&"").to_lowercase();
        if self
            .config
            .exempt_name_substrings
            .iter()
            .any(|s| name_lower.contains(s))
        {
            return true;
        }

        // Root-level dotfiles (tool configs like .ansible-lint.yml).
        if parts.len() == 1 && parts[0].starts_with('.') {
            return true;
        }

        false
    }

    /// Check if a relative path is inside a variable-definition directory.
    fn is_definition_dir(&self, rel: &str) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();
        parts
            .iter()
            .any(|part| self.config.definition_dirs.iter().any(|d| d == part))
    }
}

impl Default for MagicNumbersScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner() -> MagicNumbersScanner {
        MagicNumbersScanner::new()
    }

    #[test]
    fn test_ipv4_detection() {
        let s = scanner();
        let v = s.scan_line("  gateway: 172.26.0.1", 1, "test.yml", false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, MagicKind::Ipv4);
        assert_eq!(v[0].token, "172.26.0.1");
    }

    #[test]
    fn test_ipport_detection() {
        let s = scanner();
        let v = s.scan_line(
            "  image: 100.90.22.85:5000/localnet-agent",
            1,
            "test.yml",
            false,
        );
        assert!(v
            .iter()
            .any(|v| v.kind == MagicKind::IpPort && v.token == "100.90.22.85:5000"));
    }

    #[test]
    fn test_cidr_detection() {
        let s = scanner();
        let v = s.scan_line("  subnet: 172.26.0.0/16", 1, "test.yml", false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, MagicKind::Cidr);
    }

    #[test]
    fn test_int_detection() {
        let s = scanner();
        let v = s.scan_line("  retries: 3", 1, "test.yml", false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, MagicKind::Int);
        assert_eq!(v[0].token, "3");
    }

    #[test]
    fn test_unitnum_detection() {
        let s = scanner();
        let v = s.scan_line("  max-size: \"10m\"", 1, "test.yml", false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, MagicKind::UnitNum);
        assert_eq!(v[0].token, "10m");
    }

    #[test]
    fn test_dotted_detection() {
        let s = scanner();
        let v = s.scan_line("  version: \"3.8\"", 1, "test.yml", false);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, MagicKind::Dotted);
        assert_eq!(v[0].token, "3.8");
    }

    #[test]
    fn test_definition_dir_exempt() {
        let s = scanner();
        // In a definition dir, a direct key: value assignment is exempt.
        let v = s.scan_line("my_port: 8080", 1, "defaults/main.yml", true);
        assert_eq!(
            v.len(),
            0,
            "direct definition in defaults/ should be exempt"
        );
    }

    #[test]
    fn test_definition_dir_jinja_not_exempt() {
        let s = scanner();
        // In a definition dir, a literal inside {{ }} is still a violation.
        let v = s.scan_line(
            "  image: \"{{ local_registry | default('100.90.22.85:5000') }}/foo\"",
            1,
            "defaults/main.yml",
            true,
        );
        assert!(
            v.iter().any(|v| v.kind == MagicKind::IpPort),
            "IP inlined in Jinja in defaults/ should be flagged"
        );
    }

    #[test]
    fn test_non_definition_dir_flagged() {
        let s = scanner();
        let v = s.scan_line("  retries: 3", 1, "tasks/main.yml", false);
        assert_eq!(v.len(), 1, "literal in tasks/ should be flagged");
    }

    #[test]
    fn test_override_specific_rule() {
        let s = scanner();
        let v = s.scan_line(
            "  retries: 3  # project-lint: disable=magic-int",
            1,
            "test.yml",
            false,
        );
        assert_eq!(
            v.len(),
            0,
            "disable=magic-int should suppress int violations"
        );
    }

    #[test]
    fn test_override_multiple_rules() {
        let s = scanner();
        let v = s.scan_line(
            "  retries: 3  # project-lint: disable=magic-int,magic-unitnum",
            1,
            "test.yml",
            false,
        );
        assert_eq!(v.len(), 0, "disable=int,unitnum should suppress both");
    }

    #[test]
    fn test_override_all() {
        let s = scanner();
        let v = s.scan_line(
            "  retries: 3  # project-lint: disable=all",
            1,
            "test.yml",
            false,
        );
        assert_eq!(v.len(), 0, "disable=all should suppress everything");
    }

    #[test]
    fn test_override_bare_disable() {
        let s = scanner();
        let v = s.scan_line(
            "  retries: 3  # project-lint: disable",
            1,
            "test.yml",
            false,
        );
        assert_eq!(v.len(), 0, "bare disable should suppress everything");
    }

    #[test]
    fn test_override_specific_does_not_suppress_other_rules() {
        let s = scanner();
        let v = s.scan_line(
            "  interval: \"30s\"  # project-lint: disable=magic-int",
            1,
            "test.yml",
            false,
        );
        assert_eq!(v.len(), 1, "disable=magic-int should NOT suppress unitnum");
        assert_eq!(v[0].kind, MagicKind::UnitNum);
    }

    #[test]
    fn test_ignore_overrides_flag() {
        let s = MagicNumbersScanner::with_config(MagicNumbersConfig {
            ignore_overrides: true,
            ..MagicNumbersConfig::default_for_iac()
        });
        let v = s.scan_line(
            "  retries: 3  # project-lint: disable=all",
            1,
            "test.yml",
            false,
        );
        assert_eq!(v.len(), 1, "ignore_overrides should flag despite override");
    }

    #[test]
    fn test_strict_mode() {
        let s = MagicNumbersScanner::with_config(MagicNumbersConfig {
            strict: true,
            ..MagicNumbersConfig::default_for_iac()
        });
        let v = s.scan_line("my_port: 8080", 1, "defaults/main.yml", true);
        assert_eq!(v.len(), 1, "strict mode should flag even definitions");
    }

    #[test]
    fn test_regex_quantifier_not_flagged() {
        let s = scanner();
        let v = s.scan_line(
            "  cmd: \"grep -rE '([0-9]{1,3}\\.){3}[0-9]{1,3}' /etc\"",
            1,
            "test.yml",
            false,
        );
        // The {1,3} quantifiers should be stripped, not flagged as ints.
        assert!(
            !v.iter().any(|v| v.token == "1" || v.token == "3"),
            "regex quantifiers should not be flagged"
        );
    }

    #[test]
    fn test_shell_redirect_not_flagged() {
        let s = scanner();
        let v = s.scan_line("  cmd: \"echo hello 2>&1\"", 1, "test.yml", false);
        assert!(
            !v.iter().any(|v| v.token == "2" || v.token == "1"),
            "shell redirects should not be flagged"
        );
    }

    #[test]
    fn test_exit_code_not_flagged() {
        let s = scanner();
        let v = s.scan_line(
            "  cmd: \"curl -f http://localhost/ || exit 1\"",
            1,
            "test.yml",
            false,
        );
        assert!(
            !v.iter().any(|v| v.token == "1" && v.kind == MagicKind::Int),
            "exit codes should not be flagged"
        );
    }

    #[test]
    fn test_mac_address_flagged() {
        let s = scanner();
        let v = s.scan_line(
            "  <mac address='52:54:00:00:00:01'/>",
            1,
            "test.xml.j2",
            false,
        );
        // MAC addresses should be flagged — they are data, not syntax.
        assert!(!v.is_empty(), "MAC addresses should be flagged as data");
    }

    #[test]
    fn test_path_component_not_flagged() {
        let s = scanner();
        let v = s.scan_line(
            "  file: \"{{ playbook_dir }}/../../02-config/ansible/infrastructure/foo\"",
            1,
            "test.yml",
            false,
        );
        // The "02" in "02-config" should not be flagged (hyphen-glued).
        assert!(
            !v.iter().any(|v| v.token == "02"),
            "path components like 02-config should not be flagged"
        );
    }

    #[test]
    fn test_ipport_priority_over_ipv4() {
        // 100.90.22.85:5000 should be detected as ipport, not as ipv4 + int.
        let s = scanner();
        let v = s.scan_line("  url: 100.90.22.85:5000", 1, "test.yml", false);
        assert!(
            v.iter().any(|v| v.kind == MagicKind::IpPort),
            "IP:port should be detected as ipport, not split"
        );
        assert!(
            !v.iter().any(|v| v.kind == MagicKind::Ipv4),
            "IP:port should not also be flagged as ipv4"
        );
    }
}
