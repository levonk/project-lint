use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub files: FilesConfig,
    #[serde(default)]
    pub directories: DirectoriesConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(skip)]
    pub modular_rules: Vec<ModularRule>,
    #[serde(skip)]
    pub active_profiles: Vec<Profile>,
    #[serde(skip)]
    pub active_plugins: Vec<Plugin>,
    #[serde(skip)]
    pub core_config: CoreConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub global: GlobalConfig,
    pub profiles: ProfileConfig,
    pub plugins: PluginConfig,
    pub logging: LoggingConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub default_severity: String,
    pub output_format: String,
    pub enable_reactive_mode: bool,
    pub enable_auto_move: bool,
    pub enable_git_integration: bool,
    pub enable_file_watching: bool,
    pub max_file_size_mb: u64,
    pub scan_timeout_seconds: u64,
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub default: String,
    pub available: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub core_plugins: Vec<String>,
    pub optional_plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub include_timestamps: bool,
    pub include_rule_names: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub show_severity_icons: bool,
    pub show_rule_names: bool,
    pub show_file_paths: bool,
    pub group_by_severity: bool,
    pub max_issues_per_rule: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub metadata: ProfileMetadata,
    pub activation: ProfileActivation,
    pub enable: ProfileEnable,
    #[serde(default)]
    pub web_specific: Option<WebSpecificConfig>,
    #[serde(default)]
    pub devops_specific: Option<DevOpsSpecificConfig>,
    #[serde(default)]
    pub structure: Option<ProfileStructure>,
    #[serde(default)]
    pub extensions: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub updated: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileActivation {
    pub paths: Vec<String>,
    pub extensions: Vec<String>,
    pub branches: Vec<String>,
    pub indicators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEnable {
    pub domains: Vec<String>,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSpecificConfig {
    pub check_html_semantics: bool,
    pub validate_css_properties: bool,
    pub lint_javascript: bool,
    pub check_accessibility: bool,
    pub optimize_images: bool,
    pub check_seo_meta: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevOpsSpecificConfig {
    pub check_secrets: bool,
    pub validate_yaml: bool,
    pub check_docker_best_practices: bool,
    pub validate_terraform: bool,
    pub check_kubernetes_manifests: bool,
    pub scan_for_hardcoded_secrets: bool,
    pub check_ssl_certificates: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStructure {
    pub expected_dirs: Vec<String>,
    pub forbidden_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub metadata: PluginMetadata,
    pub trigger: PluginTrigger,
    pub execute: PluginExecute,
    #[serde(default)]
    pub pre_commit: Option<GitHookConfig>,
    #[serde(default)]
    pub pre_push: Option<GitHookConfig>,
    #[serde(default)]
    pub commit_msg: Option<GitHookConfig>,
    #[serde(default)]
    pub conditions: Option<HashMap<String, String>>,
    #[serde(default)]
    pub actions: Option<HashMap<String, String>>,
    #[serde(default)]
    pub messages: Option<HashMap<String, String>>,
    #[serde(default)]
    pub auto_move: Option<AutoMoveConfig>,
    #[serde(default)]
    pub file_detection: Option<FileDetectionConfig>,
    #[serde(default)]
    pub move_rules: Option<HashMap<String, String>>,
    #[serde(default)]
    pub safety: Option<SafetyConfig>,
    #[serde(default)]
    pub ai_config: Option<AIConfig>,
    #[serde(default)]
    pub suggestion_types: Option<SuggestionTypes>,
    #[serde(default)]
    pub context_gathering: Option<ContextGathering>,
    #[serde(default)]
    pub learning: Option<LearningConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub updated: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTrigger {
    pub on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginExecute {
    pub command: String,
    pub condition: String,
    pub timeout_seconds: u64,
    pub fail_on_errors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHookConfig {
    pub enabled: bool,
    #[serde(default)]
    pub check_staged_files: Option<bool>,
    #[serde(default)]
    pub check_unstaged_files: Option<bool>,
    #[serde(default)]
    pub auto_fix: Option<bool>,
    #[serde(default)]
    pub block_on_errors: Option<bool>,
    #[serde(default)]
    pub check_format: Option<bool>,
    #[serde(default)]
    pub require_ticket: Option<bool>,
    #[serde(default)]
    pub max_length: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMoveConfig {
    pub enabled: bool,
    pub move_on_create: bool,
    pub move_on_modify: bool,
    pub preserve_git_history: bool,
    pub create_backup: bool,
    pub dry_run_first: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDetectionConfig {
    pub use_extension: bool,
    pub use_content_analysis: bool,
    pub use_magic_numbers: bool,
    pub check_file_signatures: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub max_file_size_mb: u64,
    pub backup_enabled: bool,
    pub undo_enabled: bool,
    pub confirmation_required: bool,
    pub log_all_moves: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub enabled: bool,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub use_local_models: bool,
    pub cache_suggestions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionTypes {
    pub file_organization: bool,
    pub naming_conventions: bool,
    pub code_quality: bool,
    pub security_best_practices: bool,
    pub performance_optimization: bool,
    pub documentation_suggestions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextGathering {
    pub file_content: bool,
    pub project_structure: bool,
    pub git_history: bool,
    pub similar_files: bool,
    pub project_type: bool,
    pub team_patterns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    pub learn_from_acceptance: bool,
    pub learn_from_rejection: bool,
    pub adapt_to_project_patterns: bool,
    pub remember_user_preferences: bool,
}

// Legacy structures for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Warn when creating files on wrong branches
    #[serde(default = "default_true")]
    pub warn_wrong_branch: bool,
    /// Allowed branches for file creation
    #[serde(default)]
    pub allowed_branches: Vec<String>,
    /// Forbidden branches for file creation
    #[serde(default)]
    pub forbidden_branches: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesConfig {
    /// Auto-move files based on type
    #[serde(default = "default_true")]
    pub auto_move: bool,
    /// File type mappings
    #[serde(default)]
    pub type_mappings: HashMap<String, String>,
    /// Ignored file patterns
    #[serde(default)]
    pub ignored_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoriesConfig {
    /// Warn about scripts in wrong directory
    #[serde(default = "default_true")]
    pub warn_scripts_location: bool,
    /// Preferred scripts directory
    #[serde(default = "default_scripts_dir")]
    pub scripts_directory: String,
    /// Directory structure rules
    #[serde(default)]
    pub structure: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    /// Custom linting rules
    #[serde(default)]
    pub custom_rules: Vec<CustomRule>,
    /// Enable/disable specific checks
    #[serde(default)]
    pub enabled_checks: Vec<String>,
    #[serde(default)]
    pub disabled_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    pub name: String,
    pub pattern: String,
    pub message: String,
    pub severity: RuleSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModularRule {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub severity: RuleSeverity,
    #[serde(default)]
    pub git: Option<GitRuleConfig>,
    #[serde(default)]
    pub file_mappings: Option<HashMap<String, String>>,
    #[serde(default)]
    pub ignored_patterns: Option<HashMap<String, bool>>,
    #[serde(default)]
    pub scripts: Option<ScriptRuleConfig>,
    #[serde(default)]
    pub conditions: Option<RuleConditions>,
    #[serde(default)]
    pub messages: Option<HashMap<String, String>>,
    #[serde(default)]
    pub rules: Option<Vec<CustomRule>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRuleConfig {
    pub warn_wrong_branch: bool,
    pub allowed_branches: Vec<String>,
    pub forbidden_branches: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptRuleConfig {
    pub preferred_directory: String,
    pub alternative_directories: Vec<String>,
    pub script_extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConditions {
    pub require_git_repo: Option<bool>,
    pub check_root_scripts: Option<bool>,
    pub check_scripts_in_src: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
}

fn default_true() -> bool {
    true
}

fn default_scripts_dir() -> String {
    "bin".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            git: GitConfig::default(),
            files: FilesConfig::default(),
            directories: DirectoriesConfig::default(),
            rules: RulesConfig::default(),
            modular_rules: Vec::new(),
            active_profiles: Vec::new(),
            active_plugins: Vec::new(),
            core_config: CoreConfig::default(),
        }
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            global: GlobalConfig {
                default_severity: "warning".to_string(),
                output_format: "detailed".to_string(),
                enable_reactive_mode: true,
                enable_auto_move: true,
                enable_git_integration: true,
                enable_file_watching: true,
                max_file_size_mb: 10,
                scan_timeout_seconds: 30,
                debounce_ms: 1000,
            },
            profiles: ProfileConfig {
                default: "general".to_string(),
                available: vec![
                    "general".to_string(),
                    "web".to_string(),
                    "devops".to_string(),
                ],
            },
            plugins: PluginConfig {
                core_plugins: vec!["git-hooks".to_string(), "move-watcher".to_string()],
                optional_plugins: vec!["ai-nudge".to_string(), "format-checker".to_string()],
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "colored".to_string(),
                include_timestamps: true,
                include_rule_names: true,
            },
            output: OutputConfig {
                show_severity_icons: true,
                show_rule_names: true,
                show_file_paths: true,
                group_by_severity: true,
                max_issues_per_rule: 10,
            },
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            warn_wrong_branch: true,
            allowed_branches: vec!["main".to_string(), "master".to_string()],
            forbidden_branches: vec!["develop".to_string()],
        }
    }
}

impl Default for FilesConfig {
    fn default() -> Self {
        let mut type_mappings = HashMap::new();
        type_mappings.insert("*.sh".to_string(), "bin/".to_string());
        type_mappings.insert("*.py".to_string(), "scripts/".to_string());
        type_mappings.insert("*.js".to_string(), "scripts/".to_string());
        type_mappings.insert("*.ts".to_string(), "scripts/".to_string());

        Self {
            auto_move: true,
            type_mappings,
            ignored_patterns: vec![
                "node_modules/".to_string(),
                ".git/".to_string(),
                "target/".to_string(),
            ],
        }
    }
}

impl Default for DirectoriesConfig {
    fn default() -> Self {
        let mut structure = HashMap::new();
        structure.insert(
            "src/".to_string(),
            vec!["*.rs".to_string(), "*.py".to_string()],
        );
        structure.insert(
            "tests/".to_string(),
            vec!["*_test.*".to_string(), "*_spec.*".to_string()],
        );
        structure.insert(
            "docs/".to_string(),
            vec!["*.md".to_string(), "*.rst".to_string()],
        );

        Self {
            warn_scripts_location: true,
            scripts_directory: "bin".to_string(),
            structure,
        }
    }
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            custom_rules: vec![],
            enabled_checks: vec![
                "git_branch".to_string(),
                "file_location".to_string(),
                "directory_structure".to_string(),
            ],
            disabled_checks: vec![],
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_dir = crate::utils::get_config_dir()?;
        let config_file = config_dir.join("config.toml");

        let mut config = if config_file.exists() {
            debug!("Loading config from {:?}", config_file);
            let content = std::fs::read_to_string(&config_file)?;
            let config: Config = toml::from_str(&content)?;
            info!("Configuration loaded successfully");
            config
        } else {
            debug!("No config file found, using defaults");
            Config::default()
        };

        // Load core configuration
        config.core_config = Self::load_core_config(&config_dir)?;

        // Load modular rules from .config/project-lint/rules/active/
        config.modular_rules = Self::load_modular_rules(&config_dir)?;

        // Load profiles
        config.active_profiles = Self::load_profiles(&config_dir)?;

        // Load plugins
        config.active_plugins = Self::load_plugins(&config_dir)?;

        Ok(config)
    }

    pub fn load_core_config(config_dir: &PathBuf) -> Result<CoreConfig> {
        let core_file = config_dir.join("rules").join("core.toml");

        if core_file.exists() {
            debug!("Loading core config from {:?}", core_file);
            let content = std::fs::read_to_string(&core_file)?;
            let core_config: CoreConfig = toml::from_str(&content)?;
            info!("Core configuration loaded successfully");
            Ok(core_config)
        } else {
            debug!("No core config found, using defaults");
            Ok(CoreConfig::default())
        }
    }

    pub fn load_profiles(config_dir: &PathBuf) -> Result<Vec<Profile>> {
        let profiles_dir = config_dir.join("rules").join("profiles");
        let mut profiles = Vec::new();

        if profiles_dir.exists() {
            debug!("Loading profiles from {:?}", profiles_dir);

            for entry in WalkDir::new(&profiles_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().map_or(false, |ext| ext == "toml")
                })
            {
                let profile_path = entry.path();
                debug!("Loading profile from {:?}", profile_path);

                match std::fs::read_to_string(profile_path) {
                    Ok(content) => match toml::from_str::<Profile>(&content) {
                        Ok(profile) => {
                            let name = profile.metadata.name.clone();
                            profiles.push(profile);
                            debug!("Loaded profile: {}", name);
                        }
                        Err(e) => {
                            warn!("Failed to parse profile file {:?}: {}", profile_path, e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read profile file {:?}: {}", profile_path, e);
                    }
                }
            }
        }

        info!("Loaded {} profiles", profiles.len());
        Ok(profiles)
    }

    pub fn load_plugins(config_dir: &PathBuf) -> Result<Vec<Plugin>> {
        let plugins_dir = config_dir.join("plugins");
        let mut plugins = Vec::new();

        if plugins_dir.exists() {
            debug!("Loading plugins from {:?}", plugins_dir);

            for entry in WalkDir::new(&plugins_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().map_or(false, |ext| ext == "toml")
                })
            {
                let plugin_path = entry.path();
                debug!("Loading plugin from {:?}", plugin_path);

                match std::fs::read_to_string(plugin_path) {
                    Ok(content) => match toml::from_str::<Plugin>(&content) {
                        Ok(plugin) => {
                            let name = plugin.metadata.name.clone();
                            plugins.push(plugin);
                            debug!("Loaded plugin: {}", name);
                        }
                        Err(e) => {
                            warn!("Failed to parse plugin file {:?}: {}", plugin_path, e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read plugin file {:?}: {}", plugin_path, e);
                    }
                }
            }
        }

        info!("Loaded {} plugins", plugins.len());
        Ok(plugins)
    }

    pub fn load_modular_rules(config_dir: &PathBuf) -> Result<Vec<ModularRule>> {
        let rules_dir = config_dir.join("rules").join("active");
        let mut rules = Vec::new();

        if rules_dir.exists() {
            debug!("Loading modular rules from {:?}", rules_dir);

            for entry in WalkDir::new(&rules_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().map_or(false, |ext| ext == "toml")
                })
            {
                let rule_path = entry.path();
                debug!("Loading rule from {:?}", rule_path);

                match std::fs::read_to_string(rule_path) {
                    Ok(content) => match toml::from_str::<ModularRule>(&content) {
                        Ok(rule) => {
                            if rule.enabled {
                                let name = rule.name.clone();
                                rules.push(rule);
                                debug!("Loaded rule: {}", name);
                            } else {
                                debug!("Skipping disabled rule: {}", rule.name);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse rule file {:?}: {}", rule_path, e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read rule file {:?}: {}", rule_path, e);
                    }
                }
            }
        }

        info!("Loaded {} modular rules", rules.len());
        Ok(rules)
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = crate::utils::get_config_dir()?;
        std::fs::create_dir_all(&config_dir)?;

        let config_file = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_file, content)?;

        info!("Configuration saved to {:?}", config_file);
        Ok(())
    }

    pub fn create_default_config() -> Result<()> {
        let config = Config::default();
        config.save()?;
        Ok(())
    }
}
