pub mod config;
pub mod dependency_checker;
pub mod hooks;
pub mod profiles;
pub mod scanners;
pub mod utils;

// Re-export main types for easier access
pub use config::{
    Config, CustomRule, DevEnvironmentFilesConfig, DirectoriesConfig, DockerfileSecurityConfig,
    ExecutionMode, FilesConfig, GitConfig, GitRuleConfig, ModularRule,
    PackageManagerEnforcementConfig, RuleConditions, RuleSeverity, RulesConfig,
    RustFileNamingConfig, RustSecurityConfig, ScannerConfig, ScriptRuleConfig,
    SubmoduleIntegrityConfig, TypescriptMonorepoConfig, VaultSecurityConfig,
};
pub use hooks::{
    Decision, EventContext, EventMapper, EventType, FileEdit, HookResult, ProjectLintEvent,
    RuleEngine,
};
pub use scanners::{
    ast::{ASTAnalyzer, ASTIssue},
    ci_cd_parity::CiCdParityScanner,
    dependency_version_checker::{DependencyIssue, DependencyVersionChecker, Severity},
    detection::{
        DetectionIssue, FunctionCallDetector, FunctionCallRule, PatternDetector, PatternRule,
    },
    dev_environment::DevEnvironmentScanner,
    dockerfile_lint::DockerfileLintScanner,
    file_naming::{FileNamingScanner, NamingIssue},
    git::{check_branch_allowed, get_git_info, GitInfo},
    rust_conventions::RustConventionsScanner,
    security::{SecurityRuleSet, SecurityScanner},
    submodule_integrity::SubmoduleIntegrityScanner,
    typescript::{TypeScriptRuleSet, TypeScriptScanner},
    typescript_monorepo::TypeScriptMonorepoScanner,
    vault_security::VaultSecurityScanner,
    ScannerIssue,
};
