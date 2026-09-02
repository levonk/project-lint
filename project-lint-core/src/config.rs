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
    /// Optional per-scanner tuning sections (e.g. `[scanner_config.rust_file_naming]`).
    /// Scanners read these to augment their built-in rule sets without code changes.
    #[serde(default)]
    pub scanner_config: ScannerConfig,
    #[serde(skip)]
    pub modular_rules: Vec<ModularRule>,
    #[serde(skip)]
    pub active_profiles: Vec<Profile>,
    #[serde(skip)]
    pub active_plugins: Vec<Plugin>,
    #[serde(skip)]
    pub core_config: CoreConfig,
}

/// Container for optional per-scanner configuration sections.
///
/// Each field maps to a `[scanner_config.<section>]` TOML table. All sections
/// default to `None` so existing configs continue to parse unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScannerConfig {
    #[serde(default)]
    pub rust_file_naming: Option<RustFileNamingConfig>,
    #[serde(default)]
    pub dev_environment_files: Option<DevEnvironmentFilesConfig>,
    #[serde(default)]
    pub rust_security: Option<RustSecurityConfig>,
    #[serde(default)]
    pub vault_security: Option<VaultSecurityConfig>,
    #[serde(default)]
    pub dockerfile_security: Option<DockerfileSecurityConfig>,
    #[serde(default)]
    pub compose_lint: Option<ComposeLintConfig>,
    #[serde(default)]
    pub typescript_monorepo: Option<TypescriptMonorepoConfig>,
    #[serde(default)]
    pub package_manager_enforcement: Option<PackageManagerEnforcementConfig>,
    #[serde(default)]
    pub submodule_integrity: Option<SubmoduleIntegrityConfig>,
    #[serde(default)]
    pub magic_numbers: Option<MagicNumbersScannerConfig>,
    #[serde(default)]
    pub skill_markdown: Option<SkillMarkdownScannerConfig>,
    #[serde(default)]
    pub git_sync: Option<GitSyncScannerConfig>,
    #[serde(default)]
    pub nix_flake: Option<NixFlakeScannerConfig>,
    #[serde(default)]
    pub devbox_json: Option<DevboxJsonScannerConfig>,
    #[serde(default)]
    pub nix_shell: Option<NixShellScannerConfig>,
    #[serde(default)]
    pub envrc_content: Option<EnvrcContentScannerConfig>,
    #[serde(default)]
    pub exclusion: Option<ExclusionConfig>,
    #[serde(default)]
    pub config_validation: Option<ConfigValidationScannerConfig>,
    #[serde(default)]
    pub markdown_frontmatter: Option<MarkdownFrontmatterScannerConfig>,
    #[serde(default)]
    pub runtime_guards: Option<RuntimeGuardsScannerConfig>,
    #[serde(default)]
    pub github_workflow: Option<GithubWorkflowConfig>,
    #[serde(default)]
    pub dependabot: Option<DependabotConfig>,
    #[serde(default)]
    pub justfile_content: Option<JustfileContentConfig>,
    #[serde(default)]
    pub makefile_content: Option<MakefileContentConfig>,
    #[serde(default)]
    pub process_compose: Option<ProcessComposeConfig>,
}

/// `[scanner_config.exclusion]` — configuration for the centralized exclusion
/// list shared by all WalkDir-based scanners. Controls extra excluded paths
/// beyond the built-in defaults and the `vendor/` toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExclusionConfig {
    /// Additional directory paths to exclude (appended to
    /// `DEFAULT_EXCLUDED_DIRS`). Each entry may be a single segment
    /// (`"my-build-dir"`) or multi-segment (`"generated/src"`).
    #[serde(default)]
    pub extra_excludes: Vec<String>,

    /// When true, `vendor/` is NOT excluded (use for projects with first-party
    /// `vendor/` directories). Defaults to false.
    #[serde(default)]
    pub allow_vendor: bool,

    /// WalkDir max depth for scanners that use the shared `walk_project`
    /// helper. Defaults to 4. Individual scanners may override.
    #[serde(default = "default_exclusion_max_depth")]
    pub max_depth: usize,
}

fn default_exclusion_max_depth() -> usize {
    4
}

impl Default for ExclusionConfig {
    fn default() -> Self {
        Self {
            extra_excludes: Vec::new(),
            allow_vendor: false,
            max_depth: default_exclusion_max_depth(),
        }
    }
}

/// `[scanner_config.config_validation]` — configuration for the config file
/// validation scanner that checks tsconfig.json, eslint.config.*, tailwind
/// config, and package.json for best-practice settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValidationScannerConfig {
    /// Required ESLint base config package. Set to an empty string to disable
    /// the eslint-config-base check. Defaults to
    /// `@job-aide/tools-lint-eslint-config`.
    #[serde(default = "default_eslint_base")]
    pub required_eslint_base: String,

    /// When true, require `"type": "module"` (or any `"type"` field) in
    /// package.json. Defaults to true.
    #[serde(default = "default_true")]
    pub require_type_module: bool,

    /// When true, scan tailwind config files for extension and content rules.
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub check_tailwind: bool,
}

fn default_eslint_base() -> String {
    "@job-aide/tools-lint-eslint-config".to_string()
}

/// `[scanner_config.markdown_frontmatter]` — configuration for the markdown
/// frontmatter scanner that validates YAML frontmatter in `.md` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownFrontmatterScannerConfig {
    /// When true, warn on any `.md` file that lacks frontmatter entirely.
    /// Defaults to false (only validate frontmatter when present).
    #[serde(default)]
    pub require_frontmatter: bool,

    /// Directory prefixes where ADR-specific rules apply (adr-id, status,
    /// date, version validation). Defaults to
    /// `["internal-docs/adr", "docs-internal/adr"]` when empty.
    #[serde(default)]
    pub adr_dirs: Vec<String>,
}

/// `[scanner_config.runtime_guards]` — configuration for the runtime guards
/// scanner that detects unguarded browser API access in TS/JS files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGuardsScannerConfig {
    /// The runtime-guards package name to look for in imports. Defaults to
    /// `@job-aide/runtime-guards`.
    #[serde(default = "default_guards_package")]
    pub guards_package: String,

    /// File extensions to scan (without the dot). Defaults to
    /// `["ts", "tsx", "mts", "js", "jsx"]` when empty.
    #[serde(default)]
    pub check_extensions: Vec<String>,
}

fn default_guards_package() -> String {
    "@job-aide/runtime-guards".to_string()
}

/// `[scanner_config.nix_flake]` — configuration for the Nix flake scanner
/// that validates `flake.nix` and `flake.lock` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NixFlakeScannerConfig {
    /// When true, the `nixpkgs` input must not use `nixpkgs-unstable`.
    /// Defaults to false (unstable is allowed unless this is a production repo).
    #[serde(default)]
    pub require_stable_nixpkgs: bool,

    /// When true, check that all inputs in `flake.nix` have entries in
    /// `flake.lock`. Defaults to true.
    #[serde(default = "default_true")]
    pub check_lock_freshness: bool,
}

impl Default for NixFlakeScannerConfig {
    fn default() -> Self {
        Self {
            require_stable_nixpkgs: false,
            check_lock_freshness: default_true(),
        }
    }
}

/// `[scanner_config.devbox_json]` — configuration for the devbox.json scanner
/// that validates schema, package pinning, and script content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevboxJsonScannerConfig {
    /// When true, `devbox.json` should have a `$schema` field. Defaults to true.
    #[serde(default = "default_true")]
    pub require_schema: bool,

    /// When true, `devbox.lock` must exist when `devbox.json` exists.
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub require_lock: bool,

    /// When true, `scripts` entries should delegate to `just` targets.
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub require_scripts_use_just: bool,

    /// Commands forbidden in `scripts` and `init_hook` (e.g. `npx`, `bunx`,
    /// `yarn`). When empty, uses the scanner's built-in defaults.
    #[serde(default)]
    pub forbidden_commands: Vec<String>,
}

impl Default for DevboxJsonScannerConfig {
    fn default() -> Self {
        Self {
            require_schema: default_true(),
            require_lock: default_true(),
            require_scripts_use_just: default_true(),
            forbidden_commands: Vec::new(),
        }
    }
}

/// `[scanner_config.nix_shell]` — configuration for the Nix shell scanner
/// that validates `shell.nix` and `default.nix` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NixShellScannerConfig {
    /// When true, `shell.nix` should use `pkgs.mkShell`. Defaults to true.
    #[serde(default = "default_true")]
    pub require_mkshell: bool,

    /// When true, flag `import <nixpkgs> {}` (floating channel). Defaults to true.
    #[serde(default = "default_true")]
    pub forbid_floating_nixpkgs: bool,
}

impl Default for NixShellScannerConfig {
    fn default() -> Self {
        Self {
            require_mkshell: default_true(),
            forbid_floating_nixpkgs: default_true(),
        }
    }
}

/// `[scanner_config.envrc_content]` — configuration for the `.envrc` content
/// scanner that validates direnv configuration files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvrcContentScannerConfig {
    /// When true, `.envrc` should use `use devbox` or `use flake`. Defaults to true.
    #[serde(default = "default_true")]
    pub require_devbox: bool,

    /// When true, `.envrc` using devbox should `watch_file devbox.json`.
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub require_watch_file: bool,

    /// Regex patterns for detecting hardcoded secrets in `.envrc`. When empty,
    /// uses the scanner's built-in defaults.
    #[serde(default)]
    pub secret_patterns: Vec<String>,
}

impl Default for EnvrcContentScannerConfig {
    fn default() -> Self {
        Self {
            require_devbox: default_true(),
            require_watch_file: default_true(),
            secret_patterns: Vec::new(),
        }
    }
}

/// `[scanner_config.github_workflow]` — configuration for the GitHub Actions
/// workflow content scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubWorkflowConfig {
    #[serde(default = "default_true")]
    pub require_permissions: bool,
    #[serde(default = "default_true")]
    pub require_pinned_actions: bool,
    #[serde(default = "default_true")]
    pub require_timeout: bool,
    #[serde(default = "default_true")]
    pub require_devbox: bool,
    #[serde(default = "default_true")]
    pub forbid_pull_request_target: bool,
}

impl Default for GithubWorkflowConfig {
    fn default() -> Self {
        Self {
            require_permissions: true,
            require_pinned_actions: true,
            require_timeout: true,
            require_devbox: true,
            forbid_pull_request_target: true,
        }
    }
}

/// `[scanner_config.dependabot]` — configuration for the dependabot.yml
/// scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependabotConfig {
    #[serde(default = "default_true")]
    pub check_ecosystem_coverage: bool,
    #[serde(default)]
    pub require_group_config: bool,
}

impl Default for DependabotConfig {
    fn default() -> Self {
        Self {
            check_ecosystem_coverage: true,
            require_group_config: false,
        }
    }
}

/// `[scanner_config.justfile_content]` — configuration for the justfile
/// content scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JustfileContentConfig {
    #[serde(default = "default_true")]
    pub require_devbox_wrapper: bool,
    #[serde(default = "default_justfile_forbidden_commands")]
    pub forbidden_commands: Vec<String>,
    #[serde(default = "default_justfile_required_targets")]
    pub required_targets: Vec<String>,
}

fn default_justfile_forbidden_commands() -> Vec<String> {
    vec!["npx".to_string(), "bunx".to_string(), "yarn".to_string()]
}

fn default_justfile_required_targets() -> Vec<String> {
    vec![
        "quality".to_string(),
        "quality-full".to_string(),
        "ci".to_string(),
    ]
}

impl Default for JustfileContentConfig {
    fn default() -> Self {
        Self {
            require_devbox_wrapper: true,
            forbidden_commands: default_justfile_forbidden_commands(),
            required_targets: default_justfile_required_targets(),
        }
    }
}

/// `[scanner_config.makefile_content]` — configuration for the Makefile
/// content scanner.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MakefileContentConfig {
    #[serde(default)]
    pub require_just_delegation: bool,
}

/// `[scanner_config.process_compose]` — configuration for the
/// process-compose.yaml scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessComposeConfig {
    #[serde(default = "default_true")]
    pub require_health_check: bool,
    #[serde(default = "default_true")]
    pub require_devbox: bool,
}

impl Default for ProcessComposeConfig {
    fn default() -> Self {
        Self {
            require_health_check: true,
            require_devbox: true,
        }
    }
}

/// `[scanner_config.skill_markdown]` — configuration for the SKILL.md scanner
/// that validates the skills-src wrapper pattern: body line limit, refresh.sh
/// presence, and frontmatter validity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMarkdownScannerConfig {
    /// Maximum number of body lines (after frontmatter) allowed in a SKILL.md.
    /// Defaults to 80 when the section is absent.
    #[serde(default = "default_skill_max_body_lines")]
    pub max_body_lines: usize,

    /// When true, every SKILL.md must have a sibling `scripts/refresh.sh`.
    /// Defaults to true.
    #[serde(default = "default_true")]
    pub require_refresh_script: bool,

    /// Directory names that are fully exempt from the SKILL.md scan (e.g.
    /// `references` for bundled reference copies). When empty, only `target/`
    /// and `.git/` are skipped.
    #[serde(default)]
    pub exempt_dirs: Vec<String>,
}

fn default_skill_max_body_lines() -> usize {
    crate::scanners::skill_markdown::DEFAULT_MAX_BODY_LINES
}

/// `[scanner_config.git_sync]` — configuration for the git sync scanner that
/// warns when the local repo is behind / ahead of / diverged from its remote
/// upstream, or when the working tree has uncommitted changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSyncScannerConfig {
    /// When true, run `git fetch` before comparing local and remote refs.
    /// Defaults to true. Disable for offline or CI environments without
    /// network access.
    #[serde(default = "default_true")]
    pub fetch_before_compare: bool,

    /// Branch names treated as the project's main branch, checked against
    /// `origin/<name>`. The first name that exists locally is used. Defaults
    /// to `["main", "master"]` when empty.
    #[serde(default)]
    pub main_branches: Vec<String>,
}

/// `[scanner_config.rust_file_naming]` — extra required/forbidden files and
/// the test-file naming pattern for Rust projects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RustFileNamingConfig {
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub forbidden_files: Vec<String>,
    #[serde(default)]
    pub test_naming_pattern: Option<String>,
}

/// `[scanner_config.dev_environment_files]` — required/forbidden dev tooling
/// files (devbox, direnv, justfile, Makefile).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevEnvironmentFilesConfig {
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub forbidden_files: Vec<String>,
}

/// `[scanner_config.rust_security]` — Rust-specific security toggles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RustSecurityConfig {
    #[serde(default = "default_true")]
    pub ban_unwrap_in_lib: bool,
    #[serde(default = "default_true")]
    pub ban_unsafe_blocks: bool,
    #[serde(default)]
    pub forbidden_crates: Vec<String>,
}

/// `[scanner_config.vault_security]` — secrets management backend toggles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultSecurityConfig {
    #[serde(default)]
    pub required_env_prefix: Option<String>,
    #[serde(default)]
    pub allowed_backends: Vec<String>,
}

/// `[scanner_config.dockerfile_security]` — Dockerfile lint toggles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerfileSecurityConfig {
    #[serde(default = "default_true")]
    pub require_pinned_digests: bool,
    #[serde(default = "default_true")]
    pub require_non_root_user: bool,
    #[serde(default = "default_true")]
    pub forbid_copy_dot: bool,
    #[serde(default = "default_true")]
    pub require_healthcheck: bool,
    #[serde(default = "default_true")]
    pub require_apk_no_cache: bool,
    #[serde(default = "default_true")]
    pub require_apt_no_install_recommends: bool,
    #[serde(default = "default_true")]
    pub require_dockerignore: bool,
    #[serde(default = "default_dockerfile_exempt_digest")]
    pub exempt_from_digest_pinning: Vec<String>,
}

fn default_dockerfile_exempt_digest() -> Vec<String> {
    vec!["scratch".to_string(), "cgr.dev/chainguard".to_string()]
}

/// `[scanner_config.compose_lint]` — Compose file lint toggles for
/// `docker-compose*.yml` / `compose*.yml` files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeLintConfig {
    #[serde(default = "default_true")]
    pub require_pinned_digests: bool,
    #[serde(default = "default_true")]
    pub require_healthcheck: bool,
    #[serde(default)]
    pub require_resource_limits: bool,
    #[serde(default = "default_true")]
    pub require_no_new_privileges: bool,
    #[serde(default = "default_true")]
    pub forbid_privileged: bool,
    #[serde(default = "default_true")]
    pub forbid_docker_sock: bool,
    #[serde(default)]
    pub ops_mode: bool,
    #[serde(default = "default_compose_exempt_proxy_labels")]
    pub exempt_proxy_labels: Vec<String>,
}

fn default_compose_exempt_proxy_labels() -> Vec<String> {
    vec!["com.dockerproxy.role".to_string()]
}

/// `[scanner_config.typescript_monorepo]` — TS monorepo catalog mode, path
/// aliases, and allowed extensions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypescriptMonorepoConfig {
    #[serde(default)]
    pub catalog_mode: bool,
    #[serde(default)]
    pub path_aliases: Vec<String>,
    #[serde(default)]
    pub allowed_extensions: Vec<String>,
}

/// `[scanner_config.package_manager_enforcement]` — which package managers are
/// allowed/forbidden in a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageManagerEnforcementConfig {
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub forbidden: Vec<String>,
    #[serde(default)]
    pub required_lockfile: Option<String>,
}

/// `[scanner_config.submodule_integrity]` — toggles for the submodule
/// integrity scanner. By default the scanner checks the HEAD tree only; set
/// `check_index = true` to also inspect the staged index (pre-commit gate
/// mode).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubmoduleIntegrityConfig {
    /// When true, also inspect the staged index in addition to HEAD.
    #[serde(default)]
    pub check_index: bool,
}

/// `[scanner_config.magic_numbers]` — configuration for the magic-number
/// scanner that detects hardcoded IPs, ports, and magic numbers that should
/// be named variables.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MagicNumbersScannerConfig {
    /// Directory names where variable definitions are expected.
    /// When empty, uses the scanner's built-in defaults.
    #[serde(default)]
    pub definition_dirs: Vec<String>,

    /// Directory names that are fully exempt from scanning.
    /// When empty, uses the scanner's built-in defaults.
    #[serde(default)]
    pub exempt_dirs: Vec<String>,

    /// File extensions to scan (e.g. `[".yml", ".yaml"]`).
    /// When empty, uses the scanner's built-in defaults.
    #[serde(default)]
    pub scan_extensions: Vec<String>,

    /// Filename substrings that mark generated/lock files (always exempt).
    /// When empty, uses the scanner's built-in defaults.
    #[serde(default)]
    pub exempt_name_substrings: Vec<String>,

    /// When true, flag every literal everywhere (even in definition dirs).
    #[serde(default)]
    pub strict: bool,

    /// When true, ignore inline `# project-lint: disable=...` overrides.
    #[serde(default)]
    pub ignore_overrides: bool,
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
    pub checks: Option<ProfileChecks>,
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
    #[serde(default)]
    pub scope: String,
    pub updated: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileActivation {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(default)]
    pub indicators: Vec<String>,
    #[serde(default)]
    pub globs: Vec<String>,
    #[serde(default)]
    pub content: Vec<ContentTrigger>,
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentTrigger {
    pub matches: Vec<String>,
    #[serde(default)]
    pub globs: Vec<String>,
    #[serde(default)]
    pub position: MatchPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MatchPosition {
    Any,
    Header, // First 1024 bytes
}

impl Default for MatchPosition {
    fn default() -> Self {
        MatchPosition::Any
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEnable {
    pub domains: Vec<String>,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileChecks {
    #[serde(default)]
    pub enable: Vec<String>,
    #[serde(default)]
    pub disable: Vec<String>,
    #[serde(default)]
    pub slices: Option<ProfileSlices>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSlices {
    #[serde(default)]
    pub include: Vec<String>,
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
    #[serde(default)]
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
    #[serde(default)]
    pub mode: RulesMode,
    /// Enable/disable specific checks
    #[serde(default)]
    pub enabled_checks: Vec<String>,
    #[serde(default)]
    pub disabled_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RulesMode {
    Denylist,
    Allowlist,
}

impl Default for RulesMode {
    fn default() -> Self {
        RulesMode::Denylist
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    pub name: String,
    pub pattern: String,
    pub message: String,
    pub severity: RuleSeverity,
    #[serde(default)]
    pub check_content: bool,
    #[serde(default)]
    pub content_pattern: Option<String>,
    #[serde(default)]
    pub exception_pattern: Option<String>,
    #[serde(default)]
    pub condition: Option<String>, // e.g., "contains", "must_contain"
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub required_if_path_exists: Option<String>,
    /// Project-level kill switch. If any file matching this glob exists at the
    /// project root, the entire rule is skipped. Supports glob patterns (e.g.
    /// `"next.config.*"` to exempt Next.js/Turbopack projects from the `.ts`
    /// ban per vercel/next.js#82945). Plain paths (no glob metacharacters) are
    /// treated as literal relative paths.
    #[serde(default)]
    pub disabled_if_path_exists: Option<String>,
    /// Project-level activation gate. The rule is ONLY evaluated if a file
    /// matching this glob exists at the project root. Use to scope rules to
    /// specific project types (e.g. `"tsconfig.json"` to only apply the `.ts`
    /// ban to TypeScript projects, not Rust/Python projects that happen to
    /// have `.ts` files). Plain paths are treated as literal relative paths.
    #[serde(default)]
    pub enabled_if_path_exists: Option<String>,
    /// Per-file exclusion globs. A file that matches `pattern` but also matches
    /// any of these is not flagged by this rule. Used to exempt e.g. `*.d.ts`,
    /// `*.config.ts`, `*.test.ts` from a broad `**/*.ts` ban.
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Path globs that scope path-aware rules. Used by the
    /// `worktree-isolation-enforcer` rule: when non-empty, write-tool
    /// events are only blocked if `file_path` matches one of these globs
    /// (e.g. `["src/**"]` to protect source code but allow docs/config
    /// edits on a protected branch). Subagent dispatch is never scoped by
    /// paths — it is always blocked on a protected branch in the main
    /// worktree. When empty, the evaluator falls back to `["src/**"]`.
    #[serde(default)]
    pub protected_paths: Vec<String>,
    /// Branch names where direct work is forbidden outside a linked
    /// worktree. Used by the `worktree-isolation-enforcer` rule. When
    /// empty, the evaluator falls back to
    /// `["main", "master", "trunk", "develop"]`.
    #[serde(default)]
    pub protected_branches: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Execution mode for the rule: how matching events are dispatched.
    ///
    /// Defaults to `LocalSync` when omitted (backward compatible).
    #[serde(default)]
    pub mode: ExecutionMode,
}

/// How a matched rule is dispatched by the event router.
///
/// - `LocalSync`: evaluate synchronously in-process via the `RuleEngine`.
/// - `LocalAsync`: spawn a background task; return `Allow` immediately.
/// - `RemoteSync`: forward to a remote daemon and wait (stub — story 13-005).
/// - `RemoteAsync`: forward to a remote daemon without waiting (stub — story 12-001).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    LocalSync,
    LocalAsync,
    RemoteSync,
    RemoteAsync,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::LocalSync
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModularRule {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub severity: RuleSeverity,
    #[serde(default)]
    pub triggers: Vec<String>,
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
            scanner_config: ScannerConfig::default(),
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
            mode: RulesMode::Denylist,
            enabled_checks: vec![
                "git_branch".to_string(),
                "file_location".to_string(),
                "directory_structure".to_string(),
                "file_naming".to_string(),
                "ast_analysis".to_string(),
                "security_analysis".to_string(),
                "typescript_analysis".to_string(),
                "dependency_versions".to_string(),
                "custom_rules".to_string(),
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

    /// Load configuration from an explicit config file path.
    ///
    /// The config file's parent directory is used as the config directory for
    /// loading modular rules, profiles, and plugins (mirroring `Config::load`).
    pub fn load_from_file(config_path: &std::path::Path) -> Result<Self> {
        let config_dir = config_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let mut config = if config_path.exists() {
            debug!("Loading config from {:?}", config_path);
            let content = std::fs::read_to_string(config_path)?;
            let config: Config = toml::from_str(&content)?;
            info!("Configuration loaded successfully");
            config
        } else {
            debug!("Config file {:?} not found, using defaults", config_path);
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
        self.save_to(&config_dir)
    }

    pub fn save_to(&self, config_dir: &PathBuf) -> Result<()> {
        std::fs::create_dir_all(config_dir)?;

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

    pub fn is_check_enabled(&self, check_name: &str) -> bool {
        let effective_enabled = self.get_effective_enabled_checks();
        let effective_disabled = self.get_effective_disabled_checks();

        match self.rules.mode {
            RulesMode::Allowlist => effective_enabled.contains(check_name),
            RulesMode::Denylist => !effective_disabled.contains(check_name),
        }
    }

    fn get_effective_enabled_checks(&self) -> std::collections::HashSet<String> {
        let mut enabled = std::collections::HashSet::new();

        // Add repo rules
        for check in &self.rules.enabled_checks {
            enabled.insert(check.clone());
        }

        // Add profile rules
        for profile in &self.active_profiles {
            if let Some(checks) = &profile.checks {
                for check in &checks.enable {
                    enabled.insert(check.clone());
                }
            }
        }

        enabled
    }

    fn get_effective_disabled_checks(&self) -> std::collections::HashSet<String> {
        let mut disabled = std::collections::HashSet::new();

        // Add repo rules
        for check in &self.rules.disabled_checks {
            disabled.insert(check.clone());
        }

        // Add profile rules
        for profile in &self.active_profiles {
            if let Some(checks) = &profile.checks {
                for check in &checks.disable {
                    disabled.insert(check.clone());
                }
            }
        }

        disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_mode_default() {
        let config = RulesConfig::default();
        assert_eq!(config.mode, RulesMode::Denylist);
    }

    #[test]
    fn test_is_check_enabled_denylist() {
        let mut config = Config::default();
        config.rules.mode = RulesMode::Denylist;
        // Default disabled_checks is empty

        assert!(config.is_check_enabled("git_branch"));

        // Disable a check
        config.rules.disabled_checks.push("git_branch".to_string());
        assert!(!config.is_check_enabled("git_branch"));
        assert!(config.is_check_enabled("other_check"));
    }

    #[test]
    fn test_is_check_enabled_allowlist() {
        let mut config = Config::default();
        config.rules.mode = RulesMode::Allowlist;

        // Reset enabled checks for testing
        config.rules.enabled_checks = vec!["git_branch".to_string()];

        assert!(config.is_check_enabled("git_branch"));
        assert!(!config.is_check_enabled("other_check"));
    }

    #[test]
    fn test_profile_merging() {
        let mut config = Config::default();
        config.rules.mode = RulesMode::Allowlist;
        config.rules.enabled_checks = vec!["repo_check".to_string()];

        let profile = Profile {
            metadata: ProfileMetadata {
                name: "test".to_string(),
                version: "1.0".to_string(),
                scope: "test".to_string(),
                updated: "today".to_string(),
                description: "test".to_string(),
            },
            activation: ProfileActivation {
                paths: vec![],
                extensions: vec![],
                branches: vec![],
                indicators: vec![],
                globs: vec![],
                content: vec![],
                events: vec![],
            },
            enable: ProfileEnable {
                domains: vec![],
                plugins: vec![],
            },
            checks: Some(ProfileChecks {
                enable: vec!["profile_check".to_string()],
                disable: vec![],
                slices: None,
            }),
            web_specific: None,
            devops_specific: None,
            structure: None,
            extensions: None,
        };

        config.active_profiles.push(profile);

        assert!(config.is_check_enabled("repo_check"));
        assert!(config.is_check_enabled("profile_check"));
        assert!(!config.is_check_enabled("other_check"));
    }

    #[test]
    fn test_profile_merging_denylist() {
        let mut config = Config::default();
        config.rules.mode = RulesMode::Denylist;
        config.rules.disabled_checks = vec!["repo_disabled".to_string()];

        let profile = Profile {
            metadata: ProfileMetadata {
                name: "test".to_string(),
                version: "1.0".to_string(),
                scope: "test".to_string(),
                updated: "today".to_string(),
                description: "test".to_string(),
            },
            activation: ProfileActivation {
                paths: vec![],
                extensions: vec![],
                branches: vec![],
                indicators: vec![],
                globs: vec![],
                content: vec![],
                events: vec![],
            },
            enable: ProfileEnable {
                domains: vec![],
                plugins: vec![],
            },
            checks: Some(ProfileChecks {
                enable: vec![],
                disable: vec!["profile_disabled".to_string()],
                slices: None,
            }),
            web_specific: None,
            devops_specific: None,
            structure: None,
            extensions: None,
        };

        config.active_profiles.push(profile);

        assert!(!config.is_check_enabled("repo_disabled"));
        assert!(!config.is_check_enabled("profile_disabled"));
        assert!(config.is_check_enabled("other_check"));
    }

    #[test]
    fn test_malformed_toml_config_returns_error() {
        let bad = "[rules\nmode = \"denylist\""; // missing closing bracket
        let result: std::result::Result<Config, _> = toml::from_str(bad);
        assert!(result.is_err(), "malformed TOML should fail to parse");
    }

    #[test]
    fn test_minimal_valid_toml_config_parses() {
        let good = "";
        let config: Config = toml::from_str(good).expect("empty TOML should yield defaults");
        assert_eq!(config.rules.mode, RulesMode::Denylist);
    }

    #[test]
    fn test_custom_rule_round_trips_all_extended_fields() {
        let toml_src = r#"
[[rules.custom_rules]]
name = "ban-ts"
pattern = "**/*.ts"
message = "no ts"
severity = "warning"
check_content = false
required = false
enabled_if_path_exists = "tsconfig.json"
disabled_if_path_exists = "next.config.*"
exclude_patterns = ["**/*.d.ts"]
exception_pattern = ".*"
condition = "must_contain"
triggers = ["pre_write_code"]
mode = "local_sync"
"#;
        let config: Config = toml::from_str(toml_src).expect("round-trip");
        let rule = &config.rules.custom_rules[0];
        assert_eq!(rule.name, "ban-ts");
        assert_eq!(
            rule.enabled_if_path_exists.as_deref(),
            Some("tsconfig.json")
        );
        assert_eq!(
            rule.disabled_if_path_exists.as_deref(),
            Some("next.config.*")
        );
        assert_eq!(rule.exclude_patterns, vec!["**/*.d.ts".to_string()]);
        assert_eq!(rule.exception_pattern.as_deref(), Some(".*"));
        assert_eq!(rule.condition.as_deref(), Some("must_contain"));
        assert!(rule.triggers.iter().any(|t| t == "pre_write_code"));
        assert_eq!(rule.mode, ExecutionMode::LocalSync);
    }

    #[test]
    fn test_scanner_config_sections_parse() {
        let toml_src = r#"
[scanner_config.rust_file_naming]
required_files = ["src/lib.rs", "Cargo.lock"]
forbidden_files = ["Makefile"]
test_naming_pattern = "*_test.rs"

[scanner_config.rust_security]
ban_unwrap_in_lib = true
forbidden_crates = ["openssl"]

[scanner_config.dockerfile_security]
require_pinned_digests = true
require_non_root_user = true
forbid_copy_dot = false

[scanner_config.package_manager_enforcement]
allowed = ["pnpm"]
forbidden = ["npm", "yarn"]
required_lockfile = "pnpm-lock.yaml"
"#;
        let config: Config = toml::from_str(toml_src).expect("parse");
        let rfn = config
            .scanner_config
            .rust_file_naming
            .expect("rust_file_naming");
        assert_eq!(
            rfn.required_files,
            vec!["src/lib.rs".to_string(), "Cargo.lock".to_string()]
        );
        assert_eq!(rfn.forbidden_files, vec!["Makefile".to_string()]);
        assert_eq!(rfn.test_naming_pattern.as_deref(), Some("*_test.rs"));
        let rs = config.scanner_config.rust_security.expect("rust_security");
        assert!(rs.ban_unwrap_in_lib);
        assert_eq!(rs.forbidden_crates, vec!["openssl".to_string()]);
        let ds = config
            .scanner_config
            .dockerfile_security
            .expect("dockerfile_security");
        assert!(ds.require_pinned_digests);
        assert!(!ds.forbid_copy_dot);
        let pme = config
            .scanner_config
            .package_manager_enforcement
            .expect("pme");
        assert_eq!(pme.allowed, vec!["pnpm".to_string()]);
        assert_eq!(pme.required_lockfile.as_deref(), Some("pnpm-lock.yaml"));
    }

    #[test]
    fn test_scanner_config_defaults_to_none() {
        let config: Config = toml::from_str("").expect("empty");
        assert!(config.scanner_config.rust_file_naming.is_none());
        assert!(config.scanner_config.dockerfile_security.is_none());
    }
}
