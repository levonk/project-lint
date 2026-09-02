use colored::Colorize;
use glob::Pattern;
use project_lint_core::utils::{build_exclusions, matches_pattern, path_exists_glob, Result};
use std::path::Path;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use project_lint_core::config::{Config, ModularRule};
use project_lint_core::profiles;
use project_lint_core::scanners::ast::{ASTAnalyzer, ASTIssue};
use project_lint_core::scanners::dependency_version_checker::{
    DependencyVersionChecker, Severity as DepSeverity,
};
use project_lint_core::scanners::file_naming::FileNamingScanner;
use project_lint_core::scanners::git::{check_branch_allowed, get_git_info};
use project_lint_core::scanners::security::SecurityScanner;
use project_lint_core::scanners::typescript::TypeScriptScanner;
use project_lint_core::scanners::{
    agents_md::AgentsMdScanner, ci_cd_parity::CiCdParityScanner, compose_lint::ComposeLintScanner,
    config_validation::ConfigValidationScanner, dependabot::DependabotScanner,
    dev_environment::DevEnvironmentScanner, devbox_json::DevboxJsonScanner,
    dockerfile_lint::DockerfileLintScanner, envrc_content::EnvrcContentScanner,
    git_sync::GitSyncScanner, github_workflow::GithubWorkflowScanner,
    justfile_content::JustfileContentScanner, magic_numbers::MagicNumbersScanner,
    makefile_content::MakefileContentScanner, markdown_frontmatter::MarkdownFrontmatterScanner,
    nix_flake::NixFlakeScanner, nix_shell::NixShellScanner,
    node_modules_integrity::NodeModulesIntegrityScanner, nx_config::NxConfigScanner,
    nx_project::NxProjectScanner, path_hygiene::PathHygieneScanner,
    pnpm_workspace::PnpmWorkspaceScanner, process_compose::ProcessComposeScanner,
    runtime_guards::RuntimeGuardsScanner, rust_conventions::RustConventionsScanner,
    shell_script::ShellScriptScanner, skill_markdown::SkillMarkdownScanner,
    submodule_integrity::SubmoduleIntegrityScanner, typescript_monorepo::TypeScriptMonorepoScanner,
    vault_security::VaultSecurityScanner, ScannerIssue,
};

pub async fn run(project_path: &str, apply_fixes: bool, dry_run: bool) -> Result<()> {
    info!("Running linting checks on project: {}", project_path);

    if apply_fixes && dry_run {
        return Err(anyhow::anyhow!("Cannot use --fix and --dry-run together"));
    }

    if apply_fixes {
        info!("Fix mode enabled - violations will be automatically corrected");
    }
    if dry_run {
        info!("Dry-run mode enabled - showing what would be fixed without making changes");
    }

    let mut config = Config::load()?;
    let mut issues = Vec::new();

    // Check if project path exists
    let project_path_obj = Path::new(project_path);
    if !project_path_obj.exists() {
        return Err(anyhow::anyhow!(
            "Project path does not exist: {}",
            project_path
        ));
    }

    // Determine active profiles
    let active_profiles =
        profiles::get_active_profiles(project_path_obj, &config.active_profiles, None)?;
    if !active_profiles.is_empty() {
        info!(
            "Active profiles: {}",
            active_profiles
                .iter()
                .map(|p| p.metadata.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
                .green()
        );

        // In the future: Apply profile configurations here
        // For now, we just replace the available profiles with the active ones in the config
        config.active_profiles = active_profiles;
    } else {
        debug!("No specific profiles activated");
    }

    // Perform file naming analysis
    if config.is_check_enabled("file_naming") {
        debug!("Performing file naming analysis");
        let excluded = build_exclusions_from_config(&config);
        perform_file_naming_analysis(project_path, &mut issues, apply_fixes, dry_run, excluded)?;
    }

    // Initialize AST analyzer
    let mut ast_analyzer = ASTAnalyzer::new()?;

    // Process modular rules
    debug!("Processing {} modular rules", config.modular_rules.len());
    for rule in &config.modular_rules {
        if rule.enabled {
            process_modular_rule(project_path, rule, &mut issues, &config)?;
        }
    }

    // Perform AST-based analysis
    if config.is_check_enabled("ast_analysis") {
        debug!("Performing AST-based analysis");
        perform_ast_analysis(project_path, &mut ast_analyzer, &mut issues)?;
    }

    // Perform security scanning
    if config.is_check_enabled("security_analysis") {
        debug!("Performing security analysis");
        perform_security_analysis(project_path, &mut issues, apply_fixes, dry_run)?;
    }

    // Perform TypeScript linting
    if config.is_check_enabled("typescript_analysis") {
        debug!("Performing TypeScript analysis");
        perform_typescript_analysis(project_path, &mut issues, apply_fixes, dry_run)?;
    }

    // Perform dependency version checking
    if config.is_check_enabled("dependency_versions") {
        debug!("Performing dependency version analysis");
        perform_dependency_analysis(project_path, &mut issues, apply_fixes, dry_run).await?;
    }

    // Knowledge-bundle-driven scanners (Phase 3b). Each is gated by its own
    // check name so profiles/custom rules can disable them individually.
    if config.is_check_enabled("rust_conventions") {
        debug!("Performing rust conventions analysis");
        let excluded = build_exclusions_from_config(&config);
        perform_scanner_issues(
            "Rust",
            &RustConventionsScanner::with_exclusions(
                config
                    .scanner_config
                    .rust_security
                    .as_ref()
                    .map(|c| c.forbidden_crates.clone())
                    .unwrap_or_default(),
                excluded,
            )
            .scan(project_path)?,
            &mut issues,
        );
    }

    if config.is_check_enabled("dev_environment") {
        debug!("Performing dev environment analysis");
        let scanner = match &config.scanner_config.dev_environment_files {
            Some(c) => DevEnvironmentScanner::with_files(
                c.required_files.clone(),
                c.forbidden_files.clone(),
            ),
            None => DevEnvironmentScanner::new(),
        };
        perform_scanner_issues("DevEnv", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("ci_cd_parity") {
        debug!("Performing CI/CD parity analysis");
        perform_scanner_issues(
            "CICD",
            &CiCdParityScanner::new().scan(project_path)?,
            &mut issues,
        );
    }

    if config.is_check_enabled("dockerfile_lint") {
        debug!("Performing Dockerfile lint analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.dockerfile_security {
            Some(c) => DockerfileLintScanner::with_full_config(
                c.require_pinned_digests,
                c.require_non_root_user,
                c.forbid_copy_dot,
                c.require_healthcheck,
                c.require_apk_no_cache,
                c.require_apt_no_install_recommends,
                c.require_dockerignore,
                c.exempt_from_digest_pinning.clone(),
                excluded,
            ),
            None => DockerfileLintScanner::with_exclusions(true, true, true, excluded),
        };
        perform_scanner_issues("Docker", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("compose_lint") {
        debug!("Performing Compose lint analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.compose_lint {
            Some(c) => ComposeLintScanner::with_exclusions(
                c.require_pinned_digests,
                c.require_healthcheck,
                c.require_resource_limits,
                c.require_no_new_privileges,
                c.forbid_privileged,
                c.forbid_docker_sock,
                c.ops_mode,
                c.exempt_proxy_labels.clone(),
                excluded,
            ),
            None => ComposeLintScanner::with_exclusions(
                true,
                true,
                false,
                true,
                true,
                true,
                false,
                vec!["com.dockerproxy.role".to_string()],
                excluded,
            ),
        };
        perform_scanner_issues("Compose", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("typescript_monorepo") {
        debug!("Performing TypeScript monorepo analysis");
        let scanner = match &config.scanner_config.typescript_monorepo {
            Some(c) => {
                TypeScriptMonorepoScanner::with_config(c.catalog_mode, c.allowed_extensions.clone())
            }
            None => TypeScriptMonorepoScanner::new(),
        };
        perform_scanner_issues("TSMonorepo", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("vault_security") {
        debug!("Performing vault security analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.vault_security {
            Some(c) => VaultSecurityScanner::with_exclusions(
                c.required_env_prefix.clone(),
                c.allowed_backends.clone(),
                excluded,
            ),
            None => VaultSecurityScanner::with_exclusions(None, Vec::new(), excluded),
        };
        perform_scanner_issues("Vault", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("submodule_integrity") {
        debug!("Performing submodule integrity analysis");
        let scanner = match &config.scanner_config.submodule_integrity {
            Some(c) => SubmoduleIntegrityScanner::with_config(c.check_index),
            None => SubmoduleIntegrityScanner::new(),
        };
        perform_scanner_issues("Submodule", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("magic_numbers") {
        debug!("Performing magic-number analysis (IPs, ports, magic numbers)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.magic_numbers {
            Some(c) => {
                let mut cfg =
                    project_lint_core::scanners::magic_numbers::MagicNumbersConfig::default_for_iac(
                    );
                if !c.definition_dirs.is_empty() {
                    cfg.definition_dirs = c.definition_dirs.clone();
                }
                if !c.exempt_dirs.is_empty() {
                    cfg.exempt_dirs = c.exempt_dirs.clone();
                }
                if !c.scan_extensions.is_empty() {
                    cfg.scan_extensions = c.scan_extensions.clone();
                }
                if !c.exempt_name_substrings.is_empty() {
                    cfg.exempt_name_substrings = c.exempt_name_substrings.clone();
                }
                cfg.strict = c.strict;
                cfg.ignore_overrides = c.ignore_overrides;
                MagicNumbersScanner::with_config_and_exclusions(cfg, excluded)
            }
            None => MagicNumbersScanner::with_config_and_exclusions(
                project_lint_core::scanners::magic_numbers::MagicNumbersConfig::default_for_iac(),
                excluded,
            ),
        };
        perform_scanner_issues("MagicNum", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("skill_markdown") {
        debug!("Performing SKILL.md wrapper-pattern analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.skill_markdown {
            Some(c) => SkillMarkdownScanner::with_exclusions(
                c.max_body_lines,
                c.require_refresh_script,
                c.exempt_dirs.clone(),
                excluded,
            ),
            None => SkillMarkdownScanner::with_exclusions(
                project_lint_core::scanners::skill_markdown::DEFAULT_MAX_BODY_LINES,
                true,
                Vec::new(),
                excluded,
            ),
        };
        perform_scanner_issues("SkillMD", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("git_sync") {
        debug!("Performing git sync analysis (fetch + ahead/behind/dirty-tree)");
        let scanner = match &config.scanner_config.git_sync {
            Some(c) => GitSyncScanner::with_config(c.fetch_before_compare, c.main_branches.clone()),
            None => GitSyncScanner::new(),
        };
        perform_scanner_issues("GitSync", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("config_validation") {
        debug!("Performing config validation analysis");
        let scanner = match &config.scanner_config.config_validation {
            Some(c) => ConfigValidationScanner::with_config(
                if c.required_eslint_base.is_empty() {
                    None
                } else {
                    Some(c.required_eslint_base.clone())
                },
                c.require_type_module,
                c.check_tailwind,
            ),
            None => ConfigValidationScanner::new(),
        };
        perform_scanner_issues("ConfigVal", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("markdown_frontmatter") {
        debug!("Performing markdown frontmatter analysis");
        let scanner = match &config.scanner_config.markdown_frontmatter {
            Some(c) => {
                MarkdownFrontmatterScanner::with_config(c.require_frontmatter, c.adr_dirs.clone())
            }
            None => MarkdownFrontmatterScanner::new(),
        };
        perform_scanner_issues("MdFM", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("runtime_guards") {
        debug!("Performing runtime guards analysis");
        let scanner = match &config.scanner_config.runtime_guards {
            Some(c) => RuntimeGuardsScanner::with_config(
                c.guards_package.clone(),
                c.check_extensions.clone(),
            ),
            None => RuntimeGuardsScanner::new(),
        };
        perform_scanner_issues("RtGuards", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("nix_flake") {
        debug!("Performing nix flake analysis (flake.nix + flake.lock)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.nix_flake {
            Some(c) => NixFlakeScanner::with_exclusions(
                c.require_stable_nixpkgs,
                c.check_lock_freshness,
                excluded,
            ),
            None => NixFlakeScanner::with_exclusions(false, true, excluded),
        };
        perform_scanner_issues("NixFlake", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("devbox_json") {
        debug!("Performing devbox.json analysis (schema + pinning + scripts)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.devbox_json {
            Some(c) => DevboxJsonScanner::with_exclusions(
                c.require_schema,
                c.require_lock,
                c.require_scripts_use_just,
                c.forbidden_commands.clone(),
                excluded,
            ),
            None => DevboxJsonScanner::with_exclusions(true, true, true, Vec::new(), excluded),
        };
        perform_scanner_issues("Devbox", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("nix_shell") {
        debug!("Performing nix shell analysis (shell.nix + default.nix)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.nix_shell {
            Some(c) => NixShellScanner::with_exclusions(
                c.require_mkshell,
                c.forbid_floating_nixpkgs,
                excluded,
            ),
            None => NixShellScanner::with_exclusions(true, true, excluded),
        };
        perform_scanner_issues("NixShell", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("envrc_content") {
        debug!("Performing .envrc content analysis (secrets + devbox + paths)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.envrc_content {
            Some(c) => EnvrcContentScanner::with_exclusions(
                c.require_devbox,
                c.require_watch_file,
                c.secret_patterns.clone(),
                excluded,
            ),
            None => EnvrcContentScanner::with_exclusions(true, true, Vec::new(), excluded),
        };
        perform_scanner_issues("Envrc", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("github_workflow") {
        debug!("Performing GitHub Actions workflow analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.github_workflow {
            Some(c) => GithubWorkflowScanner::with_exclusions(
                c.require_permissions,
                c.require_pinned_actions,
                c.require_timeout,
                c.require_devbox,
                c.forbid_pull_request_target,
                excluded,
            ),
            None => GithubWorkflowScanner::with_exclusions(true, true, true, true, true, excluded),
        };
        perform_scanner_issues("GHWorkflow", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("dependabot") {
        debug!("Performing dependabot analysis");
        let scanner = match &config.scanner_config.dependabot {
            Some(c) => {
                DependabotScanner::with_config(c.check_ecosystem_coverage, c.require_group_config)
            }
            None => DependabotScanner::new(),
        };
        perform_scanner_issues("Dependabot", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("justfile_content") {
        debug!("Performing justfile content analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.justfile_content {
            Some(c) => JustfileContentScanner::with_exclusions(
                c.require_devbox_wrapper,
                c.forbidden_commands.clone(),
                c.required_targets.clone(),
                excluded,
            ),
            None => JustfileContentScanner::with_exclusions(
                true,
                vec!["npx".to_string(), "bunx".to_string(), "yarn".to_string()],
                vec![
                    "quality".to_string(),
                    "quality-full".to_string(),
                    "ci".to_string(),
                ],
                excluded,
            ),
        };
        perform_scanner_issues("Justfile", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("makefile_content") {
        debug!("Performing Makefile content analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.makefile_content {
            Some(c) => MakefileContentScanner::with_exclusions(c.require_just_delegation, excluded),
            None => MakefileContentScanner::with_exclusions(false, excluded),
        };
        perform_scanner_issues("Makefile", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("process_compose") {
        debug!("Performing process-compose analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.process_compose {
            Some(c) => ProcessComposeScanner::with_exclusions(
                c.require_health_check,
                c.require_devbox,
                excluded,
            ),
            None => ProcessComposeScanner::with_exclusions(true, true, excluded),
        };
        perform_scanner_issues("ProcCompose", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("nx_config") {
        debug!("Performing Nx config (nx.json) analysis");
        let scanner = match &config.scanner_config.nx_config {
            Some(c) => {
                NxConfigScanner::with_config(c.require_named_inputs, c.require_target_defaults)
            }
            None => NxConfigScanner::new(),
        };
        perform_scanner_issues("NxConfig", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("nx_project") {
        debug!("Performing Nx project (project.json) analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.nx_project {
            Some(c) => NxProjectScanner::with_exclusions(
                c.require_tags,
                c.require_name_matches_dir,
                excluded,
            ),
            None => NxProjectScanner::with_exclusions(false, true, excluded),
        };
        perform_scanner_issues("NxProject", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("pnpm_workspace") {
        debug!("Performing pnpm-workspace.yaml analysis");
        let scanner = match &config.scanner_config.pnpm_workspace {
            Some(c) => PnpmWorkspaceScanner::with_config(c.require_catalog, c.check_glob_matches),
            None => PnpmWorkspaceScanner::new(),
        };
        perform_scanner_issues("PnpmWs", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("node_modules_integrity") {
        debug!("Performing node_modules symlink integrity analysis");
        let scanner = match &config.scanner_config.node_modules_integrity {
            Some(c) => NodeModulesIntegrityScanner::with_config(
                c.check_symlink_structure,
                c.check_modules_yaml,
                c.check_no_foreign_lockfiles,
                c.require_package_manager_field,
            ),
            None => NodeModulesIntegrityScanner::new(),
        };
        perform_scanner_issues("NodeMods", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("shell_script") {
        debug!("Performing shell script validation (*.sh / *.bash)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.shell_script {
            Some(c) => {
                let forbidden = if c.forbidden_commands.is_empty() {
                    project_lint_core::scanners::shell_script::DEFAULT_FORBIDDEN_COMMANDS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    c.forbidden_commands.clone()
                };
                ShellScriptScanner::with_exclusions(
                    c.require_shebang,
                    c.require_strict_mode,
                    c.forbid_hardcoded_home,
                    forbidden,
                    c.require_devbox_run,
                    excluded,
                )
            }
            None => ShellScriptScanner::with_exclusions(
                true,
                true,
                true,
                project_lint_core::scanners::shell_script::DEFAULT_FORBIDDEN_COMMANDS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                false,
                excluded,
            ),
        };
        perform_scanner_issues("Shell", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("agents_md") {
        debug!("Performing AGENTS.md validation analysis");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.agents_md {
            Some(c) => AgentsMdScanner::with_exclusions(
                c.require_usage_protocol,
                c.require_jit_index,
                c.check_child_chain,
                if c.attribution_patterns.is_empty() {
                    project_lint_core::scanners::agents_md::DEFAULT_AI_ATTRIBUTION_PATTERNS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    c.attribution_patterns.clone()
                },
                excluded,
            ),
            None => AgentsMdScanner::with_exclusions(
                false,
                false,
                true,
                project_lint_core::scanners::agents_md::DEFAULT_AI_ATTRIBUTION_PATTERNS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                excluded,
            ),
        };
        perform_scanner_issues("AgentsMD", &scanner.scan(project_path)?, &mut issues);
    }

    if config.is_check_enabled("path_hygiene") {
        debug!("Performing path hygiene analysis (absolute paths, AI attribution)");
        let excluded = build_exclusions_from_config(&config);
        let scanner = match &config.scanner_config.path_hygiene {
            Some(c) => PathHygieneScanner::with_exclusions(
                c.check_absolute_home,
                c.check_ai_attribution,
                c.check_ai_signature,
                if c.exempt_files.is_empty() {
                    project_lint_core::scanners::path_hygiene::DEFAULT_EXEMPT_FILES
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    c.exempt_files.clone()
                },
                if c.attribution_patterns.is_empty() {
                    project_lint_core::scanners::path_hygiene::DEFAULT_AI_ATTRIBUTION_PATTERNS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    c.attribution_patterns.clone()
                },
                if c.signature_patterns.is_empty() {
                    project_lint_core::scanners::path_hygiene::DEFAULT_AI_SIGNATURE_PATTERNS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    c.signature_patterns.clone()
                },
                excluded,
            ),
            None => PathHygieneScanner::with_exclusions(
                true,
                true,
                true,
                project_lint_core::scanners::path_hygiene::DEFAULT_EXEMPT_FILES
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                project_lint_core::scanners::path_hygiene::DEFAULT_AI_ATTRIBUTION_PATTERNS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                project_lint_core::scanners::path_hygiene::DEFAULT_AI_SIGNATURE_PATTERNS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                excluded,
            ),
        };
        perform_scanner_issues("PathHyg", &scanner.scan(project_path)?, &mut issues);
    }

    // Legacy checks (for backward compatibility)
    if !config
        .modular_rules
        .iter()
        .any(|r| r.name == "git-branch-rules")
        && config.is_check_enabled("git_branch")
    {
        check_legacy_git_branches(project_path, &config, &mut issues)?;
    }

    if !config
        .modular_rules
        .iter()
        .any(|r| r.name == "file-organization")
        && config.is_check_enabled("file_location")
    {
        check_legacy_file_structure(project_path, &config, &mut issues)?;
    }

    if !config
        .modular_rules
        .iter()
        .any(|r| r.name == "script-location")
        && config.is_check_enabled("directory_structure")
    {
        check_legacy_directory_structure(project_path, &config, &mut issues)?;
    }

    // Report results
    if issues.is_empty() {
        println!("{}", "✓ No issues found!".green());
    } else {
        println!("{}", "Issues found:".yellow());
        for issue in &issues {
            println!("  {}", issue);
        }
        println!();
        println!("{}", format!("Found {} issue(s)", issues.len()).yellow());
    }

    Ok(())
}

fn perform_file_naming_analysis(
    project_path: &str,
    issues: &mut Vec<String>,
    apply_fixes: bool,
    dry_run: bool,
    excluded: Vec<String>,
) -> Result<()> {
    let scanner = FileNamingScanner::with_exclusions(Vec::new(), Vec::new(), excluded);

    match scanner.scan(project_path) {
        Ok(detected_issues) => {
            for issue in &detected_issues {
                let severity_icon = match issue.severity.as_str() {
                    "error" => "❌",
                    "warning" => "⚠️",
                    "info" => "ℹ️",
                    _ => "⚠️",
                };

                issues.push(format!(
                    "{} [Naming] {} (at {})",
                    severity_icon,
                    issue.message,
                    issue.path.display()
                ));
            }

            // Apply fixes if requested
            if (apply_fixes || dry_run) && !detected_issues.is_empty() {
                match scanner.apply_fixes(&detected_issues, dry_run) {
                    Ok(fixes) => {
                        if fixes > 0 {
                            if dry_run {
                                info!("📋 Would rename {} files/directories", fixes);
                            } else {
                                info!("✅ Renamed {} files/directories", fixes);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to apply naming fixes: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            debug!("Error scanning for file naming issues: {}", e);
        }
    }

    Ok(())
}

/// Build the centralized exclusion list from the `[scanner_config.exclusion]`
/// section, falling back to defaults when the section is absent.
fn build_exclusions_from_config(config: &Config) -> Vec<String> {
    match &config.scanner_config.exclusion {
        Some(c) => build_exclusions(&c.extra_excludes, c.allow_vendor),
        None => build_exclusions(&[], false),
    }
}

/// Format and append a batch of [`ScannerIssue`]s to the user-facing issue list.
/// `label` is the category prefix shown in the bracketed tag (e.g. `Rust`,
/// `DevEnv`, `CICD`).
fn perform_scanner_issues(label: &str, scanner_issues: &[ScannerIssue], issues: &mut Vec<String>) {
    for si in scanner_issues {
        let icon = match si.severity.as_str() {
            "error" => "❌",
            "warning" => "⚠️",
            "info" => "ℹ️",
            _ => "⚠️",
        };
        let loc = if si.line > 0 {
            format!("{}:{}", si.file, si.line)
        } else {
            si.file.clone()
        };
        issues.push(format!(
            "{} [{}] {} ({}: {})",
            icon, label, si.message, loc, si.rule
        ));
    }
}

fn perform_ast_analysis(
    project_path: &str,
    ast_analyzer: &mut ASTAnalyzer,
    issues: &mut Vec<String>,
) -> Result<()> {
    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let extension = path.extension().unwrap_or_default().to_string_lossy();

        // Only analyze supported file types
        if matches!(
            extension.to_lowercase().as_str(),
            "rs" | "py" | "js" | "ts" | "tsx"
        ) {
            match std::fs::read_to_string(path) {
                Ok(content) => match ast_analyzer.analyze_file(path, &content) {
                    Ok(ast_issues) => {
                        for ast_issue in ast_issues {
                            let severity_icon = match ast_issue.severity.as_str() {
                                "error" => "❌",
                                "warning" => "⚠️",
                                "info" => "ℹ️",
                                _ => "ℹ️",
                            };

                            issues.push(format!(
                                "{} {}:{}:{} - {} ({})",
                                severity_icon,
                                ast_issue.file,
                                ast_issue.line,
                                ast_issue.column,
                                ast_issue.message,
                                ast_issue.rule
                            ));
                        }
                    }
                    Err(e) => {
                        debug!("AST analysis failed for {:?}: {}", path, e);
                    }
                },
                Err(e) => {
                    debug!("Failed to read file {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(())
}

fn process_modular_rule(
    project_path: &str,
    rule: &ModularRule,
    issues: &mut Vec<String>,
    config: &Config,
) -> Result<()> {
    debug!("Processing rule: {}", rule.name);

    // Git branch rules
    if config.is_check_enabled("git_branch") {
        if let Some(git_config) = &rule.git {
            if let Some(git_info) = get_git_info(project_path)? {
                if git_config.warn_wrong_branch {
                    let branch_allowed = check_branch_allowed(
                        &git_info,
                        &git_config.allowed_branches,
                        &git_config.forbidden_branches,
                    )?;

                    if !branch_allowed {
                        let message: String = rule
                            .messages
                            .as_ref()
                            .and_then(|m| m.get("branch_not_allowed").cloned())
                            .unwrap_or_else(||
                                "⚠️  Working on branch '{branch}' which may not be appropriate for file creation".to_string()
                            );

                        issues.push(message.replace("{branch}", &git_info.current_branch));
                    }
                }
            }
        }
    }

    // File organization rules
    if config.is_check_enabled("file_location") {
        if let Some(file_mappings) = &rule.file_mappings {
            check_file_organization(project_path, file_mappings, rule, issues)?;
        }
    }

    // Script location rules
    if config.is_check_enabled("directory_structure") {
        if let Some(script_config) = &rule.scripts {
            check_script_locations(project_path, script_config, rule, issues)?;
        }
    }

    // Custom rules
    if config.is_check_enabled("custom_rules") {
        if let Some(custom_rules) = &rule.rules {
            for custom_rule in custom_rules {
                check_custom_rule(project_path, custom_rule, issues)?;
            }
        }
    }

    Ok(())
}

fn check_file_organization(
    project_path: &str,
    file_mappings: &std::collections::HashMap<String, String>,
    rule: &ModularRule,
    issues: &mut Vec<String>,
) -> Result<()> {
    let ignored_patterns = rule
        .ignored_patterns
        .as_ref()
        .map(|patterns| patterns.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix(project_path).unwrap_or(path);
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip ignored patterns
        if should_ignore_path(relative_path, &ignored_patterns) {
            continue;
        }

        // Check file mappings
        for (pattern, target_dir) in file_mappings {
            if matches_pattern(&file_name, pattern) {
                let current_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
                if current_dir.to_string_lossy() != target_dir.trim_end_matches('/') {
                    let message: String = rule
                        .messages
                        .as_ref()
                        .and_then(|m| m.get("file_misplaced").cloned())
                        .unwrap_or_else(||
                            "📁 File '{file}' should be in '{target_dir}' directory (matches pattern '{pattern}')".to_string()
                        );

                    issues.push(
                        message
                            .replace("{file}", &relative_path.display().to_string())
                            .replace("{target_dir}", target_dir)
                            .replace("{pattern}", pattern),
                    );
                }
            }
        }
    }

    Ok(())
}

fn check_script_locations(
    project_path: &str,
    script_config: &project_lint_core::config::ScriptRuleConfig,
    rule: &ModularRule,
    issues: &mut Vec<String>,
) -> Result<()> {
    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix(project_path).unwrap_or(path);
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        // Check if it's a script file
        if script_config
            .script_extensions
            .iter()
            .any(|ext| file_name.ends_with(ext))
        {
            let current_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
            let preferred_dir = script_config.preferred_directory.trim_end_matches('/');

            if current_dir.to_string_lossy() != preferred_dir {
                let message: String = rule
                    .messages
                    .as_ref()
                    .and_then(|m| m.get("script_in_wrong_location").cloned())
                    .unwrap_or_else(|| {
                        "📜 Script '{file}' should be in '{preferred_dir}' directory".to_string()
                    });

                issues.push(
                    message
                        .replace("{file}", &relative_path.display().to_string())
                        .replace("{preferred_dir}", preferred_dir),
                );
            }
        }
    }

    Ok(())
}

fn check_custom_rule(
    project_path: &str,
    custom_rule: &project_lint_core::config::CustomRule,
    issues: &mut Vec<String>,
) -> Result<()> {
    // Project-level activation gate: only evaluate if the marker exists.
    if let Some(enable_spec) = &custom_rule.enabled_if_path_exists {
        if !path_exists_glob(std::path::Path::new(project_path), enable_spec) {
            debug!(
                "Skipping rule '{}' because activation marker '{}' does not exist at project root",
                custom_rule.name, enable_spec
            );
            return Ok(());
        }
    }

    // Project-level kill switch: skip the whole rule if a matching file exists.
    if let Some(disable_spec) = &custom_rule.disabled_if_path_exists {
        if path_exists_glob(std::path::Path::new(project_path), disable_spec) {
            debug!(
                "Skipping rule '{}' because disable marker '{}' exists at project root",
                custom_rule.name, disable_spec
            );
            return Ok(());
        }
    }

    // Check conditional requirement
    if let Some(req_path) = &custom_rule.required_if_path_exists {
        if !std::path::Path::new(project_path).join(req_path).exists() {
            debug!(
                "Skipping rule '{}' because required path '{}' does not exist",
                custom_rule.name, req_path
            );
            return Ok(());
        }
    }

    let mut found_match = false;

    // Determine effective allow status for filename checks
    // If a rule is required (or conditionally required which we checked above), finding the file is generally good.
    // But if we are in denylist mode (default), finding a match is bad unless allowed.
    // If `required` is true, we want to find it.
    // If `required` is false, we don't want to find it (denylist).
    let is_allowed = custom_rule.required || custom_rule.required_if_path_exists.is_some();

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix(project_path).unwrap_or(path);
        let relative_path_str = relative_path.to_string_lossy();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        let mut matched = false;

        // Try glob matching on relative path
        if let Ok(pattern) = Pattern::new(&custom_rule.pattern) {
            if pattern.matches(&relative_path_str) {
                matched = true;
            }
        }

        // Fallback to filename matching (legacy behavior)
        if !matched && matches_pattern(&file_name, &custom_rule.pattern) {
            matched = true;
        }

        if matched {
            // Per-file exclusion: skip this file if it matches any exclude pattern.
            if !custom_rule.exclude_patterns.is_empty() {
                let mut excluded = false;
                for exclude in &custom_rule.exclude_patterns {
                    if let Ok(ex_pat) = Pattern::new(exclude) {
                        if ex_pat.matches(&relative_path_str) {
                            excluded = true;
                            break;
                        }
                    }
                    if !excluded && matches_pattern(&file_name, exclude) {
                        excluded = true;
                        break;
                    }
                }
                if excluded {
                    debug!(
                        "Rule '{}' matched '{}' but excluded by exclude_patterns",
                        custom_rule.name,
                        relative_path.display()
                    );
                    continue;
                }
            }

            found_match = true;

            // Check content if required
            if custom_rule.check_content {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let mut issue_found = false;

                    if let Some(pattern) = &custom_rule.content_pattern {
                        let contains = content.contains(pattern);

                        match custom_rule.condition.as_deref() {
                            Some("must_contain") => {
                                // Issue if content DOES NOT contain pattern
                                if !contains {
                                    issue_found = true;
                                }
                            }
                            _ => {
                                // Default: Issue if content DOES contain pattern (denylist)
                                if contains {
                                    issue_found = true;
                                }
                            }
                        }
                    } else {
                        // If check_content is true but no pattern specified
                        // If must_contain is set, and no pattern, that's weird config.
                        // Assume issue found for file existence if no pattern and default condition.
                        issue_found = true;
                    }

                    if issue_found {
                        // Check for exception pattern
                        if let Some(exception) = &custom_rule.exception_pattern {
                            if content.contains(exception) {
                                debug!("Rule '{}' matched/triggered but exception pattern found, skipping", custom_rule.name);
                                continue;
                            }
                        }

                        let severity_icon = match custom_rule.severity {
                            project_lint_core::config::RuleSeverity::Error => "❌",
                            project_lint_core::config::RuleSeverity::Warning => "⚠️",
                            project_lint_core::config::RuleSeverity::Info => "ℹ️",
                        };

                        issues.push(format!(
                            "{} {}: {} ({})",
                            severity_icon,
                            custom_rule.name,
                            custom_rule.message,
                            relative_path.display()
                        ));
                    }
                }
            } else {
                // Filename match only
                // If allowed (required or conditionally required), finding matches is good/neutral.
                // If NOT allowed (denylist), finding matches is bad.

                if !is_allowed {
                    let severity_icon = match custom_rule.severity {
                        project_lint_core::config::RuleSeverity::Error => "❌",
                        project_lint_core::config::RuleSeverity::Warning => "⚠️",
                        project_lint_core::config::RuleSeverity::Info => "ℹ️",
                    };

                    issues.push(format!(
                        "{} {}: {} ({})",
                        severity_icon,
                        custom_rule.name,
                        custom_rule.message,
                        relative_path.display()
                    ));
                }
            }
        }
    }

    // If required is true (or conditional met) and NO match found, report issue.
    // We already checked conditional requirement at the top, so if we are here, it IS required if `required` or `required_if` is set.
    // Wait. `required` field is explicit. `required_if` implies requirement if path exists.
    // So if `required` OR `required_if` is set, we expect a match.
    let expect_match = custom_rule.required || custom_rule.required_if_path_exists.is_some();

    if expect_match && !found_match {
        let severity_icon = match custom_rule.severity {
            project_lint_core::config::RuleSeverity::Error => "❌",
            project_lint_core::config::RuleSeverity::Warning => "⚠️",
            project_lint_core::config::RuleSeverity::Info => "ℹ️",
        };

        let context_msg = if let Some(req_path) = &custom_rule.required_if_path_exists {
            format!(" (Required because '{}' exists)", req_path)
        } else {
            "".to_string()
        };

        issues.push(format!(
            "{} {}: {} (Missing required file matching '{}'{})",
            severity_icon, custom_rule.name, custom_rule.message, custom_rule.pattern, context_msg
        ));
    }

    Ok(())
}

// Legacy functions for backward compatibility
fn check_legacy_git_branches(
    project_path: &str,
    config: &Config,
    issues: &mut Vec<String>,
) -> Result<()> {
    if let Some(git_info) = get_git_info(project_path)? {
        if config.git.warn_wrong_branch {
            let branch_allowed = check_branch_allowed(
                &git_info,
                &config.git.allowed_branches,
                &config.git.forbidden_branches,
            )?;

            if !branch_allowed {
                issues.push(format!(
                    "⚠️  Working on branch '{}' which may not be appropriate for file creation",
                    git_info.current_branch
                ));
            }
        }
    }
    Ok(())
}

fn check_legacy_file_structure(
    project_path: &str,
    config: &Config,
    issues: &mut Vec<String>,
) -> Result<()> {
    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix(project_path).unwrap_or(path);
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        if should_ignore_path(relative_path, &config.files.ignored_patterns) {
            continue;
        }

        if config.files.auto_move {
            for (pattern, target_dir) in &config.files.type_mappings {
                if matches_pattern(&file_name, pattern) {
                    let current_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
                    if current_dir.to_string_lossy() != target_dir.trim_end_matches('/') {
                        issues.push(format!(
                            "📁 File '{}' should be in '{}' directory (matches pattern '{}')",
                            relative_path.display(),
                            target_dir,
                            pattern
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_legacy_directory_structure(
    project_path: &str,
    config: &Config,
    issues: &mut Vec<String>,
) -> Result<()> {
    if config.directories.warn_scripts_location {
        let scripts_dir = &config.directories.scripts_directory;

        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let relative_path = path.strip_prefix(project_path).unwrap_or(path);
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            if is_script_file(&file_name) {
                let current_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
                if current_dir.to_string_lossy() != scripts_dir.trim_end_matches('/') {
                    issues.push(format!(
                        "📜 Script '{}' should be in '{}' directory",
                        relative_path.display(),
                        scripts_dir
                    ));
                }
            }
        }
    }
    Ok(())
}

fn should_ignore_path(path: &Path, ignored_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    ignored_patterns.iter().any(|pattern| {
        if pattern.ends_with('/') {
            path_str.contains(pattern.trim_end_matches('/'))
        } else {
            matches_pattern(&path_str, pattern)
        }
    })
}

fn perform_security_analysis(
    project_path: &str,
    issues: &mut Vec<String>,
    apply_fixes: bool,
    dry_run: bool,
) -> Result<()> {
    let scanner = match SecurityScanner::new() {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to initialize security scanner: {}", e);
            return Ok(());
        }
    };

    let mut security_issues = Vec::new();
    let mut total_fixes = 0;

    // Scan all source files
    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip common non-source files
        if file_name.starts_with('.')
            || file_name.ends_with(".lock")
            || file_name.ends_with(".min.js")
            || path.to_string_lossy().contains("node_modules")
            || path.to_string_lossy().contains("target")
            || path.to_string_lossy().contains(".git")
        {
            continue;
        }

        // Check if it's a source file
        let is_source = file_name.ends_with(".rs")
            || file_name.ends_with(".py")
            || file_name.ends_with(".js")
            || file_name.ends_with(".ts")
            || file_name.ends_with(".tsx")
            || file_name.ends_with(".jsx")
            || file_name.ends_with(".go")
            || file_name.ends_with(".c")
            || file_name.ends_with(".h")
            || file_name.ends_with(".cpp")
            || file_name.ends_with(".java")
            || file_name.ends_with(".cs");

        if !is_source {
            continue;
        }

        match scanner.scan_file(path) {
            Ok(detected_issues) => {
                for issue in &detected_issues {
                    security_issues.push(issue.clone());
                    issues.push(format!(
                        "🔒 [{}] {} ({}:{})",
                        issue.severity.to_uppercase(),
                        issue.message,
                        issue.file,
                        issue.line
                    ));
                }

                // Apply fixes if requested
                if (apply_fixes || dry_run) && !detected_issues.is_empty() {
                    match scanner.apply_fixes(path, &detected_issues, dry_run) {
                        Ok(fixes) => {
                            total_fixes += fixes;
                            if fixes > 0 {
                                if dry_run {
                                    info!("Would fix {} issues in {}", fixes, path.display());
                                } else {
                                    info!("Fixed {} issues in {}", fixes, path.display());
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to apply fixes to {}: {}", path.display(), e);
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Error scanning {}: {}", path.display(), e);
            }
        }
    }

    if apply_fixes && total_fixes > 0 {
        info!("✅ Applied {} security fixes", total_fixes);
    } else if dry_run && total_fixes > 0 {
        info!("📋 Would apply {} security fixes", total_fixes);
    }

    Ok(())
}

fn perform_typescript_analysis(
    project_path: &str,
    issues: &mut Vec<String>,
    apply_fixes: bool,
    dry_run: bool,
) -> Result<()> {
    let scanner = match TypeScriptScanner::new() {
        Ok(s) => s,
        Err(e) => {
            debug!("TypeScript scanner initialization failed: {}", e);
            return Ok(());
        }
    };

    let mut total_fixes = 0;

    // Scan TypeScript and JavaScript files
    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip common non-source files
        if file_name.starts_with('.')
            || file_name.ends_with(".lock")
            || file_name.ends_with(".min.js")
            || path.to_string_lossy().contains("node_modules")
            || path.to_string_lossy().contains("dist")
            || path.to_string_lossy().contains("build")
            || path.to_string_lossy().contains(".git")
        {
            continue;
        }

        // Check if it's a TypeScript/JavaScript file
        let is_ts_file = file_name.ends_with(".ts")
            || file_name.ends_with(".mts")
            || file_name.ends_with(".cts")
            || file_name.ends_with(".tsx")
            || file_name.ends_with(".js")
            || file_name.ends_with(".mjs")
            || file_name.ends_with(".cjs")
            || file_name.ends_with(".jsx")
            || file_name == "tsconfig.json"
            || file_name == "package.json"
            || file_name == "eslint.config.mts"
            || file_name == "eslint.config.ts";

        if !is_ts_file {
            continue;
        }

        match scanner.scan_file(path) {
            Ok(detected_issues) => {
                for issue in &detected_issues {
                    issues.push(format!(
                        "📘 [TypeScript] [{}] {} ({}:{})",
                        issue.severity.to_uppercase(),
                        issue.message,
                        issue.file,
                        issue.line
                    ));
                }

                // Apply fixes if requested
                if (apply_fixes || dry_run) && !detected_issues.is_empty() {
                    match scanner.apply_fixes(path, &detected_issues, dry_run) {
                        Ok(fixes) => {
                            total_fixes += fixes;
                            if fixes > 0 {
                                if dry_run {
                                    info!(
                                        "Would fix {} TypeScript issues in {}",
                                        fixes,
                                        path.display()
                                    );
                                } else {
                                    info!(
                                        "Fixed {} TypeScript issues in {}",
                                        fixes,
                                        path.display()
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            debug!(
                                "Failed to apply TypeScript fixes to {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Error scanning TypeScript file {}: {}", path.display(), e);
            }
        }
    }

    if apply_fixes && total_fixes > 0 {
        info!("✅ Applied {} TypeScript fixes", total_fixes);
    } else if dry_run && total_fixes > 0 {
        info!("📋 Would apply {} TypeScript fixes", total_fixes);
    }

    Ok(())
}

fn is_script_file(file_name: &str) -> bool {
    let script_extensions = [".sh", ".py", ".js", ".ts", ".rb", ".pl", ".php"];
    script_extensions.iter().any(|ext| file_name.ends_with(ext))
}

async fn perform_dependency_analysis(
    project_path: &str,
    issues: &mut Vec<String>,
    apply_fixes: bool,
    dry_run: bool,
) -> Result<()> {
    let checker = DependencyVersionChecker::new();

    match checker.scan(project_path).await {
        Ok(detected_issues) => {
            for issue in &detected_issues {
                let severity_icon = match issue.severity {
                    DepSeverity::Error => "🔴",
                    DepSeverity::Warning => "🟡",
                    DepSeverity::Info => "🟢",
                };

                issues.push(format!(
                    "{} [Dependencies] {} ({})",
                    severity_icon, issue.message, issue.file_path
                ));
            }

            // Apply fixes if requested
            if (apply_fixes || dry_run) && !detected_issues.is_empty() {
                match checker.apply_fixes(&detected_issues, dry_run).await {
                    Ok(fixes) => {
                        if fixes > 0 {
                            if dry_run {
                                info!("📋 Would update {} dependencies", fixes);
                            } else {
                                info!("✅ Updated {} dependencies", fixes);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to apply dependency fixes: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            debug!("Error checking dependency versions: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_lint_core::config::{CustomRule, ExecutionMode, RuleSeverity};
    use std::fs;
    use tempfile::TempDir;

    fn ban_ts_rule() -> CustomRule {
        CustomRule {
            name: "ban_ambiguous_ts".to_string(),
            pattern: "**/*.ts".to_string(),
            message: "Ambiguous .ts".to_string(),
            severity: RuleSeverity::Warning,
            check_content: false,
            content_pattern: None,
            exception_pattern: None,
            condition: None,
            required: false,
            required_if_path_exists: None,
            disabled_if_path_exists: None,
            enabled_if_path_exists: None,
            exclude_patterns: vec![
                "**/*.d.ts".to_string(),
                "**/*.config.ts".to_string(),
                "**/*.test.ts".to_string(),
            ],
            protected_paths: vec![],
            protected_branches: vec![],
            triggers: vec![],
            mode: ExecutionMode::LocalSync,
        }
    }

    #[test]
    fn test_exclude_patterns_exempt_d_ts() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("types.d.ts"), "export {};\n")?;
        fs::write(dir.path().join("foo.ts"), "export {};\n")?;

        let rule = ban_ts_rule();
        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // foo.ts should be flagged; types.d.ts should be exempt.
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("foo.ts"));
        assert!(!issues[0].contains("types.d.ts"));
        Ok(())
    }

    #[test]
    fn test_exclude_patterns_exempt_config_and_test_ts() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("vitest.config.ts"), "export {};\n")?;
        fs::write(dir.path().join("foo.test.ts"), "export {};\n")?;
        fs::write(dir.path().join("bar.ts"), "export {};\n")?;

        let rule = ban_ts_rule();
        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // Only bar.ts should be flagged.
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("bar.ts"));
        Ok(())
    }

    #[test]
    fn test_disabled_if_path_exists_next_config() -> Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("next.config.mjs"), "export {};\n")?;
        fs::create_dir_all(dir.path().join("app"))?;
        fs::write(dir.path().join("app/page.ts"), "export {};\n")?;
        fs::create_dir_all(dir.path().join("lib"))?;
        fs::write(dir.path().join("lib/utils.ts"), "export {};\n")?;

        let mut rule = ban_ts_rule();
        rule.disabled_if_path_exists = Some("next.config.*".to_string());

        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // Next.js project: .ts ban is disabled entirely.
        assert!(
            issues.is_empty(),
            "expected no issues in Next.js project, got: {:?}",
            issues
        );
        Ok(())
    }

    #[test]
    fn test_disabled_if_path_exists_no_marker_still_flags() -> Result<()> {
        let dir = TempDir::new()?;
        fs::create_dir_all(dir.path().join("lib"))?;
        fs::write(dir.path().join("lib/utils.ts"), "export {};\n")?;

        let mut rule = ban_ts_rule();
        rule.disabled_if_path_exists = Some("next.config.*".to_string());

        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // No next.config -> rule active -> utils.ts flagged.
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("utils.ts"));
        Ok(())
    }

    #[test]
    fn test_enabled_if_path_exists_skips_non_ts_project() -> Result<()> {
        let dir = TempDir::new()?;
        // No tsconfig.json -> NOT a TypeScript project -> rule skipped.
        fs::create_dir_all(dir.path().join("src"))?;
        fs::write(dir.path().join("src/utils.ts"), "export {};\n")?;
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"foo\"\n")?;

        let mut rule = ban_ts_rule();
        rule.enabled_if_path_exists = Some("tsconfig.json".to_string());

        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // Non-TS project (no tsconfig.json) -> rule not activated -> no issues.
        assert!(
            issues.is_empty(),
            "expected no issues in non-TS project, got: {:?}",
            issues
        );
        Ok(())
    }

    #[test]
    fn test_enabled_if_path_exists_activates_for_ts_project() -> Result<()> {
        let dir = TempDir::new()?;
        // tsconfig.json present -> TypeScript project -> rule active.
        fs::create_dir_all(dir.path().join("src"))?;
        fs::write(dir.path().join("src/utils.ts"), "export {};\n")?;
        fs::write(dir.path().join("tsconfig.json"), "{}\n")?;

        let mut rule = ban_ts_rule();
        rule.enabled_if_path_exists = Some("tsconfig.json".to_string());

        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // TS project -> rule active -> utils.ts flagged.
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("utils.ts"));
        Ok(())
    }

    #[test]
    fn test_enabled_and_disabled_gates_combine() -> Result<()> {
        let dir = TempDir::new()?;
        // TS project (tsconfig.json) BUT also Next.js (next.config.mjs) -> disabled wins.
        fs::create_dir_all(dir.path().join("app"))?;
        fs::write(dir.path().join("app/page.ts"), "export {};\n")?;
        fs::write(dir.path().join("tsconfig.json"), "{}\n")?;
        fs::write(dir.path().join("next.config.mjs"), "export {};\n")?;

        let mut rule = ban_ts_rule();
        rule.enabled_if_path_exists = Some("tsconfig.json".to_string());
        rule.disabled_if_path_exists = Some("next.config.*".to_string());

        let mut issues = Vec::new();
        check_custom_rule(&dir.path().to_string_lossy(), &rule, &mut issues)?;

        // Both gates: enabled (tsconfig exists) AND disabled (next.config exists) -> disabled wins.
        assert!(
            issues.is_empty(),
            "expected no issues in Next.js TS project, got: {:?}",
            issues
        );
        Ok(())
    }
}
