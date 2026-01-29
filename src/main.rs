use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod ast;
mod commands;
mod config;
mod config_validation;
mod dependency_checker;
mod dependency_version_checker;
mod detection;
mod hooks;
mod file_naming;
mod git;
mod markdown_frontmatter;
mod package_organization;
mod pnpm_lockfile;
mod profiles;
mod runtime_guards;
mod security;
mod typescript;
mod utils;

use crate::utils::Result;
use commands::{init, lint, watch};

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
        /// Apply automatic fixes to detected issues
        #[arg(long)]
        fix: bool,
        /// Show what would be fixed without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Watch for file changes and run linting automatically
    Watch {
        /// Path to the project root (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Run as a hook handler for IDE events
    Hook(commands::hook::HookArgs),
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

    let _subscriber = FmtSubscriber::builder()
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
        Commands::Lint { path, fix, dry_run } => {
            let project_path = path.unwrap_or_else(|| ".".to_string());
            lint::run(&project_path, fix, dry_run).await?;
        }
        Commands::Watch { path } => {
            let project_path = path.unwrap_or_else(|| ".".to_string());
            watch::run(&project_path).await?;
        }
        Commands::Hook(args) => {
            commands::hook::run(args).await?;
        }
    }

    Ok(())
}
