use clap::Args;
use project_lint_core::config::Config;
use project_lint_core::hooks::{
    initialize_global_logger, log_hook_event,
    mappers::{ClaudeMapper, KiroMapper, WindsurfMapper},
    Decision, EventMapper, HookResult, RuleEngine,
};
use project_lint_core::profiles;
use project_lint_core::utils::Result;
use serde_json::json;
use std::io::{self, Read};
use std::path::Path;
use std::time::Instant;
use tracing::{debug, error, info, warn};

#[derive(Args)]
pub struct HookArgs {
    /// Source of the hook (windsurf, claude, kiro, generic)
    #[arg(long, default_value = "windsurf")]
    pub source: String,

    /// Path to the project root (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<String>,

    /// Output the decision as a structured JSON object in addition to the IDE-specific response
    #[arg(long)]
    pub json: bool,

    /// Path to the config file (overrides project-local discovery)
    #[arg(long)]
    pub config_file: Option<String>,

    /// Session identifier included in hook log entries
    #[arg(long)]
    pub session_id: Option<String>,

    /// Project identifier included in hook log entries
    #[arg(long)]
    pub project_id: Option<String>,
}

pub async fn run(args: HookArgs) -> Result<()> {
    // Initialize hook logger
    initialize_global_logger(None)?;

    let start_time = Instant::now();
    // Read stdin
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    if buffer.is_empty() {
        debug!("Empty hook input, skipping");
        return Ok(());
    }

    debug!("Received hook input from {}: {}", args.source, buffer);

    // Select mapper
    let mapper: Box<dyn EventMapper> = match args.source.to_lowercase().as_str() {
        "windsurf" => Box::new(WindsurfMapper),
        "claude" => Box::new(ClaudeMapper),
        "kiro" => Box::new(KiroMapper),
        _ => {
            warn!(
                "Unknown source '{}', defaulting to Windsurf mapper",
                args.source
            );
            Box::new(WindsurfMapper)
        }
    };

    // Parse event
    let mut event = match mapper.map_event(&buffer) {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to parse hook event: {}", e);
            return Ok(());
        }
    };

    // Override session_id from CLI flag if provided
    if let Some(session_id) = &args.session_id {
        event.session_id = Some(session_id.clone());
    }

    info!("Processing event: {:?}", event.event_type);

    // Load config
    let project_path_str = args.path.unwrap_or_else(|| ".".to_string());
    let project_path = Path::new(&project_path_str);

    let mut config = match &args.config_file {
        Some(config_file) => {
            let config_path = Path::new(config_file);
            Config::load_from_file(config_path)?
        }
        None => Config::load()?,
    };

    // Determine active profiles for this event
    let active_profiles =
        profiles::get_active_profiles(project_path, &config.active_profiles, Some(&event))?;
    config.active_profiles = active_profiles;

    // Evaluate rules
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event)?;

    // Output IDE-specific response
    let output = mapper.format_response(result.clone())?;
    if !output.is_empty() {
        println!("{}", output);
    }

    // Capture message for logging before potentially moving it for JSON output
    let log_message = result.message.clone();

    // Output structured JSON if requested
    if args.json {
        let decision_str = match result.decision {
            Decision::Allow => "allow",
            Decision::Warn => "warn",
            Decision::Deny => "deny",
            Decision::Ask => "ask",
        };
        let json_output = json!({
            "decision": decision_str,
            "modified_input": result.modified_input.unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            "message": result.message.unwrap_or_default(),
        });
        println!("{}", json_output);
    }

    // Log the hook event
    let duration_ms = start_time.elapsed().as_millis() as u64;
    let decision_str = format!("{:?}", result.decision);
    let message_str = log_message.as_deref();

    // Attach project_id to the event context for logging if provided
    if let Some(project_id) = &args.project_id {
        // Store project_id in the original payload so it can be logged
        if let Some(payload) = &mut event.context.original_payload {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "project_id".to_string(),
                    serde_json::Value::String(project_id.clone()),
                );
            } else {
                let mut map = serde_json::Map::new();
                map.insert(
                    "project_id".to_string(),
                    serde_json::Value::String(project_id.clone()),
                );
                *payload = serde_json::Value::Object(map);
            }
        } else {
            let mut map = serde_json::Map::new();
            map.insert(
                "project_id".to_string(),
                serde_json::Value::String(project_id.clone()),
            );
            event.context.original_payload = Some(serde_json::Value::Object(map));
        }
    }

    if let Err(e) = log_hook_event(&event, &decision_str, message_str, Some(duration_ms)) {
        error!("Failed to log hook event: {}", e);
    }

    // Handle blocking (exit code 2 is standard for blocking in many agent hook systems)
    if result.decision == Decision::Deny {
        std::process::exit(2);
    }

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

    #[derive(clap::Subcommand)]
    enum TestCommands {
        Hook(HookArgs),
    }

    #[test]
    fn test_hook_args_defaults() {
        let cli = TestCli::parse_from(["test", "hook"]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert_eq!(args.source, "windsurf");
                assert!(!args.json);
                assert!(args.config_file.is_none());
                assert!(args.session_id.is_none());
                assert!(args.project_id.is_none());
            }
        }
    }

    #[test]
    fn test_hook_args_json_flag() {
        let cli = TestCli::parse_from(["test", "hook", "--json"]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert!(args.json);
            }
        }
    }

    #[test]
    fn test_hook_args_config_file_flag() {
        let cli = TestCli::parse_from(["test", "hook", "--config-file", "/tmp/config.toml"]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert_eq!(args.config_file.as_deref(), Some("/tmp/config.toml"));
            }
        }
    }

    #[test]
    fn test_hook_args_session_id_flag() {
        let cli = TestCli::parse_from(["test", "hook", "--session-id", "sess-123"]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert_eq!(args.session_id.as_deref(), Some("sess-123"));
            }
        }
    }

    #[test]
    fn test_hook_args_project_id_flag() {
        let cli = TestCli::parse_from(["test", "hook", "--project-id", "proj-456"]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert_eq!(args.project_id.as_deref(), Some("proj-456"));
            }
        }
    }

    #[test]
    fn test_hook_args_source_flag() {
        let cli = TestCli::parse_from(["test", "hook", "--source", "claude"]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert_eq!(args.source, "claude");
            }
        }
    }

    #[test]
    fn test_hook_args_all_flags_combined() {
        let cli = TestCli::parse_from([
            "test",
            "hook",
            "--source",
            "claude",
            "--json",
            "--config-file",
            "/tmp/c.toml",
            "--session-id",
            "s1",
            "--project-id",
            "p1",
        ]);
        match cli.command {
            TestCommands::Hook(args) => {
                assert_eq!(args.source, "claude");
                assert!(args.json);
                assert_eq!(args.config_file.as_deref(), Some("/tmp/c.toml"));
                assert_eq!(args.session_id.as_deref(), Some("s1"));
                assert_eq!(args.project_id.as_deref(), Some("p1"));
            }
        }
    }
}
