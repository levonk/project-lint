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

impl From<ProjectLintError> for anyhow::Error {
    fn from(err: ProjectLintError) -> Self {
        anyhow::anyhow!(err)
    }
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
