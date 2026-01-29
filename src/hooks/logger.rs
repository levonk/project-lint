use crate::hooks::ProjectLintEvent;
use crate::utils::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookLogEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub source: String,
    pub session_id: Option<String>,
    pub file_path: Option<String>,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub decision: String,
    pub message: Option<String>,
    pub duration_ms: Option<u64>,
}

pub struct HookLogger {
    log_file: PathBuf,
}

impl HookLogger {
    pub fn new(log_dir: Option<PathBuf>) -> Result<Self> {
        let log_dir = log_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
                .join("project-lint")
                .join("logs")
        });

        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&log_dir)?;

        // Create log file with current date
        let now = Utc::now();
        let log_file_name = format!("hook-log-{}.jsonl", now.format("%Y-%m-%d"));
        let log_file = log_dir.join(log_file_name);

        info!("Hook logging to: {:?}", log_file);

        Ok(Self { log_file })
    }

    pub fn log_event(&self, event: &ProjectLintEvent, decision: &str, message: Option<&str>, duration_ms: Option<u64>) -> Result<()> {
        let entry = HookLogEntry {
            timestamp: Utc::now(),
            event_type: format!("{:?}", event.event_type),
            source: event.context.ide_source.clone(),
            session_id: event.session_id.clone(),
            file_path: event.context.file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            tool_name: event.context.tool_name.clone(),
            command: event.context.command.clone(),
            decision: decision.to_string(),
            message: message.map(|s| s.to_string()),
            duration_ms,
        };

        let line = serde_json::to_string(&entry)? + "\n";

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(line.as_bytes()) {
                    error!("Failed to write hook log: {}", e);
                } else {
                    debug!("Logged hook event: {:?}", entry.event_type);
                }
            }
            Err(e) => {
                error!("Failed to open hook log file: {}", e);
            }
        }

        Ok(())
    }

    pub fn get_recent_logs(&self, limit: Option<usize>) -> Result<Vec<HookLogEntry>> {
        if !self.log_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.log_file)?;
        let lines: Vec<&str> = content.lines().collect();

        let start_idx = if let Some(limit) = limit {
            if lines.len() > limit {
                lines.len() - limit
            } else {
                0
            }
        } else {
            0
        };

        let mut entries = Vec::new();
        for line in lines.iter().skip(start_idx) {
            if let Ok(entry) = serde_json::from_str::<HookLogEntry>(line) {
                entries.push(entry);
            } else {
                warn!("Failed to parse log line: {}", line);
            }
        }

        Ok(entries)
    }

    pub fn get_stats(&self) -> Result<HookStats> {
        let entries = self.get_recent_logs(None)?;

        let mut stats = HookStats::default();

        for entry in entries {
            stats.total_events += 1;

            // Count by event type
            *stats.event_counts.entry(entry.event_type.clone()).or_insert(0) += 1;

            // Count by source
            *stats.source_counts.entry(entry.source.clone()).or_insert(0) += 1;

            // Count by decision
            *stats.decision_counts.entry(entry.decision.clone()).or_insert(0) += 1;

            // Track duration
            if let Some(duration) = entry.duration_ms {
                stats.total_duration_ms += duration;
                stats.event_count_with_duration += 1;

                if duration > stats.max_duration_ms {
                    stats.max_duration_ms = duration;
                }

                if stats.min_duration_ms == 0 || duration < stats.min_duration_ms {
                    stats.min_duration_ms = duration;
                }
            }
        }

        Ok(stats)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookStats {
    pub total_events: u64,
    pub event_counts: std::collections::HashMap<String, u64>,
    pub source_counts: std::collections::HashMap<String, u64>,
    pub decision_counts: std::collections::HashMap<String, u64>,
    pub total_duration_ms: u64,
    pub event_count_with_duration: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
}

impl HookStats {
    pub fn average_duration_ms(&self) -> f64 {
        if self.event_count_with_duration == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.event_count_with_duration as f64
        }
    }
}

// Global logger instance (using lazy_static or once_cell would be better in production)
static mut GLOBAL_LOGGER: Option<HookLogger> = None;
static LOGGER_INITIALIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn initialize_global_logger(log_dir: Option<PathBuf>) -> Result<()> {
    if LOGGER_INITIALIZED.compare_exchange(false, true, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed).is_ok() {
        unsafe {
            GLOBAL_LOGGER = Some(HookLogger::new(log_dir)?);
        }
        info!("Global hook logger initialized");
    }
    Ok(())
}

pub fn log_hook_event(event: &ProjectLintEvent, decision: &str, message: Option<&str>, duration_ms: Option<u64>) -> Result<()> {
    unsafe {
        if let Some(logger) = &GLOBAL_LOGGER {
            logger.log_event(event, decision, message, duration_ms)
        } else {
            // Fallback: create a temporary logger
            let logger = HookLogger::new(None)?;
            logger.log_event(event, decision, message, duration_ms)
        }
    }
}

pub fn get_hook_stats() -> Result<HookStats> {
    unsafe {
        if let Some(logger) = &GLOBAL_LOGGER {
            logger.get_stats()
        } else {
            // Fallback: create a temporary logger
            let logger = HookLogger::new(None)?;
            logger.get_stats()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod logger_tests;
}
