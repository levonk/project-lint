use crate::utils::Result;
use crate::hooks::{EventMapper, HookResult, Decision, RuleEngine, mappers::{WindsurfMapper, ClaudeMapper}};
use crate::config::Config;
use crate::profiles;
use clap::Args;
use std::io::{self, Read};
use std::path::Path;
use tracing::{debug, error, info, warn};

#[derive(Args)]
pub struct HookArgs {
    /// Source of the hook (windsurf, claude, kiro, generic)
    #[arg(long, default_value = "windsurf")]
    pub source: String,

    /// Path to the project root (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<String>,
}

pub async fn run(args: HookArgs) -> Result<()> {
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
            warn!("Unknown source '{}', defaulting to Windsurf mapper", args.source);
            Box::new(WindsurfMapper)
        },
    };

    // Parse event
    let event = match mapper.map_event(&buffer) {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to parse hook event: {}", e);
            return Ok(());
        }
    };

    info!("Processing event: {:?}", event.event_type);

    // Load config
    let project_path_str = args.path.unwrap_or_else(|| ".".to_string());
    let project_path = Path::new(&project_path_str);

    let mut config = Config::load()?;

    // Determine active profiles for this event
    let active_profiles = profiles::get_active_profiles(project_path, &config.active_profiles, Some(&event))?;
    config.active_profiles = active_profiles;

    // Evaluate rules
    let engine = RuleEngine::new(&config);
    let result = engine.evaluate_event(&event)?;

    // Output response
    let output = mapper.format_response(result.clone())?;
    if !output.is_empty() {
        println!("{}", output);
    }

    // Handle blocking (exit code 2 is standard for blocking in many agent hook systems)
    if result.decision == Decision::Deny {
        std::process::exit(2);
    }

    Ok(())
}
