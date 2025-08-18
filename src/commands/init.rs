use colored::Colorize;
use tracing::{info, warn};
use utils::Result;

use crate::config::Config;

pub async fn run(force: bool) -> Result<()> {
    info!("Initializing project-lint configuration");

    let config_dir = utils::get_config_dir()?;
    let config_file = config_dir.join("config.toml");

    if config_file.exists() && !force {
        warn!("Configuration file already exists at {:?}", config_file);
        println!("{}", "Configuration file already exists!".yellow());
        println!("Use --force to overwrite the existing configuration.");
        return Ok(());
    }

    // Create default configuration
    Config::create_default_config()?;

    println!("{}", "✓ Project-lint initialized successfully!".green());
    println!("Configuration created at: {:?}", config_file);
    println!();
    println!("You can now:");
    println!("  • Run 'project-lint lint' to check your project");
    println!("  • Run 'project-lint watch' to monitor file changes");
    println!("  • Edit the configuration file to customize rules");

    Ok(())
}
