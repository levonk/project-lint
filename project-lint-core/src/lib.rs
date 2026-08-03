pub mod config;
pub mod dependency_checker;
pub mod hooks;
pub mod profiles;
pub mod scanners;
pub mod utils;

// Re-export main types for easier access
pub use config::{
    Config, CustomRule, DirectoriesConfig, ExecutionMode, FilesConfig, GitConfig, GitRuleConfig,
    ModularRule, RuleConditions, RuleSeverity, RulesConfig, ScriptRuleConfig,
};
pub use hooks::{
    Decision, EventContext, EventMapper, EventType, FileEdit, HookResult, ProjectLintEvent,
    RuleEngine,
};
pub use scanners::{
    ast::{ASTAnalyzer, ASTIssue},
    detection::{DetectionIssue, FunctionCallDetector, FunctionCallRule, PatternDetector, PatternRule},
    file_naming::{FileNamingScanner, NamingIssue},
    git::{check_branch_allowed, get_git_info, GitInfo},
    security::{SecurityRuleSet, SecurityScanner},
    typescript::{TypeScriptRuleSet, TypeScriptScanner},
    dependency_version_checker::{DependencyIssue, DependencyVersionChecker, Severity},
};
