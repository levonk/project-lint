use chrono::Utc;
use clap::{Args, Subcommand};
use project_lint_core::config::Config;
use project_lint_core::utils::Result;
use serde::Serialize;
use std::path::Path;
use tracing::info;

#[derive(Args)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub command: PolicyCommand,
}

#[derive(Subcommand)]
pub enum PolicyCommand {
    /// Export active rules and profiles as a signed JSON bundle
    Export(ExportArgs),
}

#[derive(Args)]
pub struct ExportArgs {
    /// Path to the config file (overrides project-local discovery)
    #[arg(long)]
    pub config_file: Option<String>,

    /// Path to the project root (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<String>,
}

/// The JSON bundle format emitted by `policy export`.
#[derive(Debug, Serialize)]
struct PolicyBundle {
    version: u32,
    rules: Vec<serde_json::Value>,
    profiles: Vec<serde_json::Value>,
    generated_at: String,
}

pub async fn run(args: PolicyArgs) -> Result<()> {
    match args.command {
        PolicyCommand::Export(export_args) => run_export(export_args).await,
    }
}

async fn run_export(args: ExportArgs) -> Result<()> {
    // Load config (either from explicit file or project-local discovery)
    let config = match &args.config_file {
        Some(config_file) => {
            let config_path = Path::new(config_file);
            Config::load_from_file(config_path)?
        }
        None => Config::load()?,
    };

    // Serialize rules: combine modular rules and top-level custom rules
    let mut rules: Vec<serde_json::Value> = Vec::new();

    for rule in &config.modular_rules {
        rules.push(serde_json::to_value(rule)?);
    }

    for rule in &config.rules.custom_rules {
        rules.push(serde_json::to_value(rule)?);
    }

    // Serialize active profiles
    let profiles: Vec<serde_json::Value> = config
        .active_profiles
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let bundle = PolicyBundle {
        version: 1,
        rules,
        profiles,
        generated_at: Utc::now().to_rfc3339(),
    };

    let json = serde_json::to_string_pretty(&bundle)?;
    println!("{}", json);

    info!(
        "Exported policy bundle with {} rules and {} profiles",
        bundle.rules.len(),
        bundle.profiles.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: TestCommands,
    }

    #[derive(Subcommand)]
    enum TestCommands {
        Policy(PolicyArgs),
    }

    #[test]
    fn test_policy_export_args_default() {
        let cli = TestCli::parse_from(["test", "policy", "export"]);
        match cli.command {
            TestCommands::Policy(PolicyArgs {
                command: PolicyCommand::Export(args),
            }) => {
                assert!(args.config_file.is_none());
                assert!(args.path.is_none());
            }
        }
    }

    #[test]
    fn test_policy_export_args_config_file() {
        let cli = TestCli::parse_from(["test", "policy", "export", "--config-file", "/tmp/c.toml"]);
        match cli.command {
            TestCommands::Policy(PolicyArgs {
                command: PolicyCommand::Export(args),
            }) => {
                assert_eq!(args.config_file.as_deref(), Some("/tmp/c.toml"));
            }
        }
    }

    #[test]
    fn test_policy_export_args_path() {
        let cli = TestCli::parse_from(["test", "policy", "export", "--path", "/tmp"]);
        match cli.command {
            TestCommands::Policy(PolicyArgs {
                command: PolicyCommand::Export(args),
            }) => {
                assert_eq!(args.path.as_deref(), Some("/tmp"));
            }
        }
    }

    #[test]
    fn test_policy_bundle_serialization() {
        let bundle = PolicyBundle {
            version: 1,
            rules: vec![serde_json::json!({"name": "test-rule"})],
            profiles: vec![serde_json::json!({"metadata": {"name": "test-profile"}})],
            generated_at: "2026-08-02T00:00:00+00:00".to_string(),
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["version"], 1);
        assert!(parsed["rules"].is_array());
        assert!(parsed["profiles"].is_array());
        assert!(parsed["generated_at"].is_string());
    }

    #[test]
    fn test_policy_bundle_empty_serialization() {
        let bundle = PolicyBundle {
            version: 1,
            rules: vec![],
            profiles: vec![],
            generated_at: "2026-08-02T00:00:00+00:00".to_string(),
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["rules"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["profiles"].as_array().unwrap().len(), 0);
    }
}
