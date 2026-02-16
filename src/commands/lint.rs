use crate::utils::{Result, matches_pattern};
use colored::Colorize;
use std::path::Path;
use tracing::{debug, info, warn};
use walkdir::WalkDir;
use glob::Pattern;

use crate::ast::{ASTAnalyzer, ASTIssue};
use crate::config::{Config, ModularRule};
use crate::dependency_version_checker::DependencyVersionChecker;
use crate::git::{check_branch_allowed, get_git_info};
use crate::profiles;
use crate::security::SecurityScanner;
use crate::typescript::TypeScriptScanner;
use crate::file_naming::FileNamingScanner;

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
    let active_profiles = profiles::get_active_profiles(project_path_obj, &config.active_profiles, None)?;
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
        perform_file_naming_analysis(project_path, &mut issues, apply_fixes, dry_run)?;
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
) -> Result<()> {
    let scanner = FileNamingScanner::new();

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
    // Check conditional requirement
    if let Some(req_path) = &custom_rule.required_if_path_exists {
        if !std::path::Path::new(project_path).join(req_path).exists() {
            debug!("Skipping rule '{}' because required path '{}' does not exist", custom_rule.name, req_path);
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
                            crate::config::RuleSeverity::Error => "❌",
                            crate::config::RuleSeverity::Warning => "⚠️",
                            crate::config::RuleSeverity::Info => "ℹ️",
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
                        crate::config::RuleSeverity::Error => "❌",
                        crate::config::RuleSeverity::Warning => "⚠️",
                        crate::config::RuleSeverity::Info => "ℹ️",
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
            crate::config::RuleSeverity::Error => "❌",
            crate::config::RuleSeverity::Warning => "⚠️",
            crate::config::RuleSeverity::Info => "ℹ️",
        };

        let context_msg = if let Some(req_path) = &custom_rule.required_if_path_exists {
            format!(" (Required because '{}' exists)", req_path)
        } else {
            "".to_string()
        };

        issues.push(format!(
            "{} {}: {} (Missing required file matching '{}'{})",
            severity_icon,
            custom_rule.name,
            custom_rule.message,
            custom_rule.pattern,
            context_msg
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
                    crate::dependency_version_checker::Severity::Error => "🔴",
                    crate::dependency_version_checker::Severity::Warning => "🟡",
                    crate::dependency_version_checker::Severity::Info => "🟢",
                };

                issues.push(format!(
                    "{} [Dependencies] {} ({})",
                    severity_icon,
                    issue.message,
                    issue.file_path
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
