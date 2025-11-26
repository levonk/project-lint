use crate::utils::Result;
use colored::Colorize;
use std::path::Path;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::ast::{ASTAnalyzer, ASTIssue};
use crate::config::{Config, ModularRule};
use crate::git::{check_branch_allowed, get_git_info};
use crate::profiles;
use crate::security::SecurityScanner;
use crate::typescript::TypeScriptScanner;

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
    let active_profiles = profiles::get_active_profiles(project_path_obj, &config.active_profiles)?;
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

    // Initialize AST analyzer
    let mut ast_analyzer = ASTAnalyzer::new()?;

    // Process modular rules
    debug!("Processing {} modular rules", config.modular_rules.len());
    for rule in &config.modular_rules {
        if rule.enabled {
            process_modular_rule(project_path, rule, &mut issues)?;
        }
    }

    // Perform AST-based analysis
    debug!("Performing AST-based analysis");
    perform_ast_analysis(project_path, &mut ast_analyzer, &mut issues)?;

    // Perform security scanning
    debug!("Performing security analysis");
    perform_security_analysis(project_path, &mut issues, apply_fixes, dry_run)?;

    // Perform TypeScript linting
    debug!("Performing TypeScript analysis");
    perform_typescript_analysis(project_path, &mut issues, apply_fixes, dry_run)?;

    // Legacy checks (for backward compatibility)
    if !config
        .modular_rules
        .iter()
        .any(|r| r.name == "git-branch-rules")
    {
        check_legacy_git_branches(project_path, &config, &mut issues)?;
    }

    if !config
        .modular_rules
        .iter()
        .any(|r| r.name == "file-organization")
    {
        check_legacy_file_structure(project_path, &config, &mut issues)?;
    }

    if !config
        .modular_rules
        .iter()
        .any(|r| r.name == "script-location")
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
) -> Result<()> {
    debug!("Processing rule: {}", rule.name);

    // Git branch rules
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

    // File organization rules
    if let Some(file_mappings) = &rule.file_mappings {
        check_file_organization(project_path, file_mappings, rule, issues)?;
    }

    // Script location rules
    if let Some(script_config) = &rule.scripts {
        check_script_locations(project_path, script_config, rule, issues)?;
    }

    // Custom rules
    if let Some(custom_rules) = &rule.rules {
        for custom_rule in custom_rules {
            check_custom_rule(project_path, custom_rule, issues)?;
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
    script_config: &crate::config::ScriptRuleConfig,
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
    custom_rule: &crate::config::CustomRule,
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

        if matches_pattern(&file_name, &custom_rule.pattern) {
            let severity_icon = match custom_rule.severity {
                crate::config::RuleSeverity::Error => "❌",
                crate::config::RuleSeverity::Warning => "⚠️",
                crate::config::RuleSeverity::Info => "ℹ️",
            };

            issues.push(format!(
                "{} {}: {}",
                severity_icon, custom_rule.name, custom_rule.message
            ));
        }
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

fn matches_pattern(file_name: &str, pattern: &str) -> bool {
    if pattern.starts_with('*') && pattern.ends_with('*') {
        file_name.contains(&pattern[1..pattern.len() - 1])
    } else if pattern.starts_with('*') {
        file_name.ends_with(&pattern[1..])
    } else if pattern.ends_with('*') {
        file_name.starts_with(&pattern[..pattern.len() - 1])
    } else {
        file_name == pattern
    }
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
                for issue in detected_issues {
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
                for issue in detected_issues {
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
                                    info!("Would fix {} TypeScript issues in {}", fixes, path.display());
                                } else {
                                    info!("Fixed {} TypeScript issues in {}", fixes, path.display());
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Failed to apply TypeScript fixes to {}: {}", path.display(), e);
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
