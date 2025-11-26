pub mod ast;
pub mod commands;
pub mod config;
pub mod detection;
pub mod git;
pub mod profiles;
pub mod security;
pub mod utils;

// Re-export main types for easier testing
pub use ast::{ASTAnalyzer, ASTIssue};
pub use config::{
    Config, CustomRule, DirectoriesConfig, FilesConfig, GitConfig, GitRuleConfig, ModularRule,
    RuleConditions, RuleSeverity, RulesConfig, ScriptRuleConfig,
};
pub use git::GitInfo;
