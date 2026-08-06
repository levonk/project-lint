pub mod commands;
pub mod pnpm_lockfile;

// Re-export core modules for backward compatibility
pub use project_lint_core::{config, dependency_checker, hooks, profiles, scanners, utils};

// Re-export main types for easier testing
pub use project_lint_core::config::{
    Config, CustomRule, DirectoriesConfig, FilesConfig, GitConfig, GitRuleConfig, ModularRule,
    RuleConditions, RuleSeverity, RulesConfig, ScriptRuleConfig,
};
pub use project_lint_core::scanners::ast::{ASTAnalyzer, ASTIssue};
pub use project_lint_core::scanners::git::GitInfo;
