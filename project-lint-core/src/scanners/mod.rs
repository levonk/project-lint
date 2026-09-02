pub mod agents_md;
pub mod ast;
pub mod ci_cd_parity;
pub mod config_validation;
pub mod dependency_version_checker;
pub mod detection;
pub mod dev_environment;
pub mod dockerfile_lint;
pub mod file_naming;
pub mod git;
pub mod git_sync;
pub mod magic_numbers;
pub mod markdown_frontmatter;
pub mod package_organization;
pub mod path_hygiene;
pub mod runtime_guards;
pub mod rust_conventions;
pub mod security;
pub mod skill_markdown;
pub mod submodule_integrity;
pub mod typescript;
pub mod typescript_monorepo;
pub mod vault_security;

/// Generic issue emitted by the project-level scanners (rust_conventions,
/// dev_environment, ci_cd_parity, dockerfile_lint, typescript_monorepo,
/// vault_security, skill_markdown). Carries enough context for the lint command
/// to format a human-readable line and for tests to assert on
/// severity/file/message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannerIssue {
    pub file: String,
    pub line: usize,
    pub severity: String,
    pub rule: String,
    pub message: String,
}

impl ScannerIssue {
    pub fn new(rule: &str, severity: &str, file: &str, message: impl Into<String>) -> Self {
        Self {
            file: file.to_string(),
            line: 0,
            severity: severity.to_string(),
            rule: rule.to_string(),
            message: message.into(),
        }
    }

    pub fn at_line(mut self, line: usize) -> Self {
        self.line = line;
        self
    }
}
