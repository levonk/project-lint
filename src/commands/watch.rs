use colored::Colorize;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, info, warn};
use utils::Result;

use crate::config::Config;

pub async fn run(project_path: &str) -> Result<()> {
    info!("Starting file watcher for project: {}", project_path);

    let config = Config::load()?;

    // Check if project path exists
    if !Path::new(project_path).exists() {
        return Err(anyhow::anyhow!(
            "Project path does not exist: {}",
            project_path
        ));
    }

    println!("{}", "🔍 Watching for file changes...".blue());
    println!("Press Ctrl+C to stop watching");
    println!();

    // Create a channel to receive the events.
    let (tx, rx) = mpsc::channel();

    // Create a watcher object, delivering debounced events.
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(Path::new(project_path), RecursiveMode::Recursive)?;

    let mut last_check = std::time::Instant::now();
    let debounce_duration = Duration::from_millis(1000); // 1 second debounce

    for res in rx {
        match res {
            Ok(event) => {
                debug!("File system event: {:?}", event);

                // Debounce events to avoid too frequent checks
                let now = std::time::Instant::now();
                if now.duration_since(last_check) > debounce_duration {
                    last_check = now;

                    // Run linting checks
                    println!("{}", "📝 File change detected, running checks...".yellow());
                    if let Err(e) = run_lint_checks(project_path, &config).await {
                        warn!("Error during linting: {}", e);
                    }
                    println!();
                }
            }
            Err(e) => {
                warn!("Watch error: {:?}", e);
            }
        }
    }

    Ok(())
}

async fn run_lint_checks(project_path: &str, config: &Config) -> Result<()> {
    let mut issues = Vec::new();

    // Git branch checks
    if let Some(git_info) = crate::git::get_git_info(project_path)? {
        if config.git.warn_wrong_branch {
            let branch_allowed = crate::git::check_branch_allowed(
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

    // Quick file structure check (only for recent changes)
    check_recent_file_changes(project_path, config, &mut issues)?;

    // Report results
    if issues.is_empty() {
        println!("{}", "✓ No issues found!".green());
    } else {
        println!("{}", "Issues found:".yellow());
        for issue in &issues {
            println!("  {}", issue);
        }
    }

    Ok(())
}

fn check_recent_file_changes(
    project_path: &str,
    config: &Config,
    issues: &mut Vec<String>,
) -> Result<()> {
    use std::path::Path;
    use walkdir::WalkDir;

    let now = std::time::SystemTime::now();
    let recent_threshold = Duration::from_secs(60); // Check files modified in last minute

    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Check if file was recently modified
        if let Ok(metadata) = path.metadata() {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = now.duration_since(modified) {
                    if duration < recent_threshold {
                        let relative_path = path.strip_prefix(project_path).unwrap_or(path);
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

                        // Skip ignored patterns
                        if should_ignore_path(relative_path, &config.files.ignored_patterns) {
                            continue;
                        }

                        // Check if file should be moved based on type
                        if config.files.auto_move {
                            for (pattern, target_dir) in &config.files.type_mappings {
                                if matches_pattern(&file_name, pattern) {
                                    let current_dir =
                                        relative_path.parent().unwrap_or_else(|| Path::new(""));
                                    if current_dir.to_string_lossy()
                                        != target_dir.trim_end_matches('/')
                                    {
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

                        // Check for scripts in wrong location
                        if config.directories.warn_scripts_location && is_script_file(&file_name) {
                            let scripts_dir = &config.directories.scripts_directory;
                            let current_dir =
                                relative_path.parent().unwrap_or_else(|| Path::new(""));
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

fn is_script_file(file_name: &str) -> bool {
    let script_extensions = [".sh", ".py", ".js", ".ts", ".rb", ".pl", ".php"];
    script_extensions.iter().any(|ext| file_name.ends_with(ext))
}
