use crate::utils::Result;
use colored::Colorize;
use tracing::{info, warn};

use crate::config::Config;

pub async fn run(force: bool) -> Result<()> {
    info!("Initializing project-lint configuration");

    // Initialize in the current project root
    let project_root = crate::utils::get_project_root()?;
    let config_dir = project_root.join(".config").join("project-lint");
    let config_file = config_dir.join("config.toml");

    if config_file.exists() && !force {
        warn!("Configuration file already exists at {:?}", config_file);
        println!("{}", "Configuration file already exists!".yellow());
        println!("Use --force to overwrite the existing configuration.");
        return Ok(());
    }

    // Create default configuration
    let config = Config::default();
    config.save_to(&config_dir)?;

    println!("{}", "✓ Project-lint initialized successfully!".green());
    println!("Configuration created at: {:?}", config_file);
    println!();
    println!("You can now:");
    println!("  • Run 'project-lint lint' to check your project");
    println!("  • Run 'project-lint watch' to monitor file changes");
    println!("  • Edit the configuration file to customize rules");

    Ok(())
}
