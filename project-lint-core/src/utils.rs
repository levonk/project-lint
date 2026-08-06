use anyhow::Result as AnyhowResult;
use thiserror::Error;

pub type Result<T> = AnyhowResult<T>;

#[derive(Error, Debug)]
pub enum ProjectLintError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

pub fn get_project_root() -> Result<std::path::PathBuf> {
    let current_dir = std::env::current_dir()?;

    // Walk up the directory tree to find a git repository
    let mut path = current_dir.clone();
    while path.parent().is_some() {
        if path.join(".git").exists() {
            return Ok(path);
        }
        path = path.parent().unwrap().to_path_buf();
    }

    Err(anyhow::anyhow!(
        "No git repository found in current directory or parents"
    ))
}

pub fn get_config_dir() -> Result<std::path::PathBuf> {
    // First try project-specific config
    let project_root = get_project_root()?;
    let project_config = project_root.join(".config").join("project-lint");
    if project_config.exists() {
        return Ok(project_config);
    }

    // Fallback to XDG config home
    if let Some(config_dir) = dirs::config_dir() {
        let xdg_config = config_dir.join("project-lint");
        return Ok(xdg_config);
    }

    // Final fallback to ~/.config/project-lint
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home.join(".config").join("project-lint"))
}

pub fn matches_pattern(file_name: &str, pattern: &str) -> bool {
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

/// Returns true if `pattern` contains glob metacharacters (`*`, `?`, `[`).
pub fn is_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

/// Check whether any file matching `spec` exists relative to `project_root`.
///
/// `spec` may be a plain relative path (e.g. `"Cargo.toml"`) or a glob pattern
/// (e.g. `"next.config.*"`). For globs, only direct children of the project
/// root are considered (depth-1 match), which is the common case for config
/// files. Plain paths are checked with a direct filesystem stat.
pub fn path_exists_glob(project_root: &std::path::Path, spec: &str) -> bool {
    if !is_glob(spec) {
        return project_root.join(spec).exists();
    }

    // Glob: match direct children of project_root.
    if let Ok(entries) = std::fs::read_dir(project_root) {
        if let Ok(pattern) = glob::Pattern::new(spec) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if pattern.matches(&name_str) {
                    return true;
                }
            }
        }
    }
    false
}
