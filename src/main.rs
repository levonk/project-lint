use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

mod ast;
mod commands;
mod config;
mod git;
mod utils;

use crate::utils::Result;
use commands::{init, lint, watch};
use config::Config;

#[derive(Parser)]
#[command(
    name = "project-lint",
    about = "A CLI tool for project linting and file organization",
    version,
    long_about = "Project-lint helps you maintain clean project structure by warning about file placement, git branch issues, and automatically organizing files based on type."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize project-lint configuration
    Init {
        /// Force overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },
    /// Run linting checks on the current project
    Lint {
        /// Path to the project root (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Watch for file changes and run linting automatically
    Watch {
        /// Path to the project root (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .init();

    info!("Starting project-lint");

    match cli.command {
        Commands::Init { force } => {
            init::run(force).await?;
        }
        Commands::Lint { path } => {
            let project_path = path.unwrap_or_else(|| ".".to_string());
            lint::run(&project_path).await?;
        }
        Commands::Watch { path } => {
            let project_path = path.unwrap_or_else(|| ".".to_string());
            watch::run(&project_path).await?;
        }
    }

    Ok(())
}
