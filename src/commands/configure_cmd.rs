use crate::commands::configure::run_tui;
use crate::config::Config;
use crate::utils::Result;
use std::path::PathBuf;
use tracing::{info, error};

pub async fn run() -> Result<()> {
    info!("Starting project-lint configuration TUI");
    
    // Load configuration
    let config = Config::load()?;
    let config_path = get_config_path();
    
    // Run TUI
    match run_tui(config, config_path) {
        Ok(_) => {
            info!("Configuration TUI completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Configuration TUI failed: {}", e);
            Err(e)
        }
    }
}

fn get_config_path() -> Option<PathBuf> {
    // Try to find the config file in standard locations
    let paths = vec![
        dirs::home_dir()?.join(".config").join("project-lint").join("config.toml"),
        PathBuf::from(".config").join("project-lint").join("config.toml"),
        PathBuf::from("project-lint.toml"),
    ];
    
    for path in paths {
        if path.exists() {
            return Some(path);
        }
    }
    
    // Default to user config path
    dirs::home_dir().map(|h| h.join(".config").join("project-lint").join("config.toml"))
}
