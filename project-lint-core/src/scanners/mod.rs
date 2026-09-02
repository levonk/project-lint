pub mod agents_md;
pub mod ansible_lint;
pub mod ast;
pub mod binary_validation;
pub mod ci_cd_parity;
pub mod compose_lint;
pub mod config_validation;
pub mod dependabot;
pub mod dependency_version_checker;
pub mod detection;
pub mod dev_environment;
pub mod devbox_json;
pub mod dockerfile_lint;
pub mod envrc_content;
pub mod file_naming;
pub mod git;
pub mod git_sync;
pub mod github_workflow;
pub mod go_config;
pub mod gradle_config;
pub mod jinja_template;
pub mod justfile_content;
pub mod magic_numbers;
pub mod makefile_content;
pub mod markdown_frontmatter;
pub mod nix_flake;
pub mod nix_shell;
pub mod node_modules_integrity;
pub mod nx_config;
pub mod nx_project;
pub mod package_organization;
pub mod path_hygiene;
pub mod pnpm_workspace;
pub mod prisma_schema;
pub mod process_compose;
pub mod protobuf_lint;
pub mod pulumi_lint;
pub mod python_config;
pub mod runtime_guards;
pub mod rust_conventions;
pub mod security;
pub mod shell_script;
pub mod skill_markdown;
pub mod sql_migration;
pub mod submodule_integrity;
pub mod terraform_lint;
pub mod typescript;
pub mod typescript_monorepo;
pub mod vault_security;

/// Generic issue emitted by the project-level scanners (rust_conventions,
/// dev_environment, ci_cd_parity, dockerfile_lint, compose_lint,
/// typescript_monorepo, vault_security, skill_markdown, nix_flake,
/// devbox_json, nix_shell, envrc_content, github_workflow, dependabot,
/// justfile_content, makefile_content, process_compose, nx_config,
/// nx_project, pnpm_workspace, node_modules_integrity, shell_script,
/// agents_md, path_hygiene, python_config, go_config, gradle_config,
/// terraform_lint, pulumi_lint, ansible_lint, jinja_template,
/// sql_migration, protobuf_lint, prisma_schema, binary_validation). Carries enough
/// context for the lint command to format a human-readable line and for tests
/// to assert on severity/file/message.
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
