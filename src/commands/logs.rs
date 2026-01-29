use crate::utils::Result;
use crate::hooks::{get_hook_stats, HookLogger};
use clap::Args;
use std::path::PathBuf;
use tracing::info;

#[derive(Args)]
pub struct LogsArgs {
    /// Number of recent entries to show (default: 50)
    #[arg(short, long, default_value = "50")]
    pub limit: usize,
    
    /// Show statistics only
    #[arg(long)]
    pub stats: bool,
    
    /// Log directory path (default: ~/.local/share/project-lint/logs)
    #[arg(short, long)]
    pub dir: Option<String>,
}

pub async fn run(args: LogsArgs) -> Result<()> {
    let log_dir = args.dir.map(PathBuf::from);
    let logger = HookLogger::new(log_dir)?;
    
    if args.stats {
        show_stats(&logger).await?;
    } else {
        show_recent_logs(&logger, args.limit).await?;
    }
    
    Ok(())
}

async fn show_stats(logger: &HookLogger) -> Result<()> {
    let stats = get_hook_stats()?;
    
    println!("📊 Hook Statistics");
    println!("================");
    println!("Total events: {}", stats.total_events);
    
    if stats.event_count_with_duration > 0 {
        println!("Average duration: {:.2}ms", stats.average_duration_ms());
        println!("Min duration: {}ms", stats.min_duration_ms);
        println!("Max duration: {}ms", stats.max_duration_ms);
    }
    
    println!();
    println!("📈 Events by Type:");
    for (event_type, count) in &stats.event_counts {
        println!("  {}: {}", event_type, count);
    }
    
    println!();
    println!("🔌 Events by Source:");
    for (source, count) in &stats.source_counts {
        println!("  {}: {}", source, count);
    }
    
    println!();
    println!("⚖️  Events by Decision:");
    for (decision, count) in &stats.decision_counts {
        println!("  {}: {}", decision, count);
    }
    
    Ok(())
}

async fn show_recent_logs(logger: &HookLogger, limit: usize) -> Result<()> {
    let entries = logger.get_recent_logs(Some(limit))?;
    
    if entries.is_empty() {
        println!("No hook logs found.");
        return Ok(());
    }
    
    println!("📋 Recent Hook Logs (showing {} entries)", entries.len());
    println!("================");
    
    for entry in entries {
        let decision_icon = match entry.decision.as_str() {
            "Allow" => "✅",
            "Deny" => "❌",
            "Warn" => "⚠️",
            "Ask" => "❓",
            _ => "•",
        };
        
        println!("{} {} [{}] {}", 
            decision_icon,
            entry.timestamp.format("%H:%M:%S"),
            entry.source,
            entry.event_type
        );
        
        if let Some(session_id) = &entry.session_id {
            println!("  Session: {}", session_id);
        }
        
        if let Some(tool_name) = &entry.tool_name {
            println!("  Tool: {}", tool_name);
        }
        
        if let Some(command) = &entry.command {
            println!("  Command: {}", command);
        }
        
        if let Some(message) = &entry.message {
            // Truncate very long messages
            let truncated = if message.len() > 100 {
                format!("{}...", &message[..97])
            } else {
                message.clone()
            };
            println!("  Message: {}", truncated);
        }
        
        if let Some(duration) = entry.duration_ms {
            println!("  Duration: {}ms", duration);
        }
        
        println!();
    }
    
    Ok(())
}
