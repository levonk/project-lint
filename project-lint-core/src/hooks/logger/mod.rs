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

    pub fn log_event(
        &self,
        event: &ProjectLintEvent,
        decision: &str,
        message: Option<&str>,
        duration_ms: Option<u64>,
    ) -> Result<()> {
        let entry = HookLogEntry {
            timestamp: Utc::now(),
            event_type: format!("{:?}", event.event_type),
            source: event.context.ide_source.clone(),
            session_id: event.session_id.clone(),
            file_path: event
                .context
                .file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
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
            *stats
                .event_counts
                .entry(entry.event_type.clone())
                .or_insert(0) += 1;

            // Count by source
            *stats.source_counts.entry(entry.source.clone()).or_insert(0) += 1;

            // Count by decision
            *stats
                .decision_counts
                .entry(entry.decision.clone())
                .or_insert(0) += 1;

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
static LOGGER_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn initialize_global_logger(log_dir: Option<PathBuf>) -> Result<()> {
    if LOGGER_INITIALIZED
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
        )
        .is_ok()
    {
        unsafe {
            GLOBAL_LOGGER = Some(HookLogger::new(log_dir)?);
        }
        info!("Global hook logger initialized");
    }
    Ok(())
}

pub fn log_hook_event(
    event: &ProjectLintEvent,
    decision: &str,
    message: Option<&str>,
    duration_ms: Option<u64>,
) -> Result<()> {
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
    use crate::hooks::{EventContext, EventType, ProjectLintEvent};
    use crate::utils::Result;
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_hook_logger_create() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let logger = HookLogger::new(Some(temp_dir.path().to_path_buf()))?;

        // Check that log directory was created
        assert!(temp_dir.path().exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_log_event() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let logger = HookLogger::new(Some(temp_dir.path().to_path_buf()))?;

        // Create a test event
        let event = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: Some("test-session".to_string()),
            timestamp: Some("2025-01-28T20:00:00Z".to_string()),
            cwd: Some(PathBuf::from("/test")),
            context: EventContext {
                file_path: Some(PathBuf::from("/test/file.rs")),
                tool_name: Some("bash".to_string()),
                command: Some("npm install".to_string()),
                ide_source: "test".to_string(),
                ..Default::default()
            },
        };

        // Log the event
        logger.log_event(&event, "Allow", Some("Test message"), Some(10))?;

        // Read back the logs
        let entries = logger.get_recent_logs(Some(1))?;
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.event_type, "PreToolUse");
        assert_eq!(entry.source, "test");
        assert_eq!(entry.decision, "Allow");
        assert_eq!(entry.message, Some("Test message".to_string()));
        assert_eq!(entry.duration_ms, Some(10));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_recent_logs_with_limit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let logger = HookLogger::new(Some(temp_dir.path().to_path_buf()))?;

        // Log multiple events
        for i in 0..5 {
            let event = ProjectLintEvent {
                event_type: EventType::PreToolUse,
                session_id: Some(format!("session-{}", i)),
                timestamp: None,
                cwd: None,
                context: EventContext {
                    ide_source: "test".to_string(),
                    ..Default::default()
                },
            };

            logger.log_event(&event, "Allow", None, Some(i))?;
        }

        // Get last 3 entries
        let entries = logger.get_recent_logs(Some(3))?;
        assert_eq!(entries.len(), 3);

        // Should be the last 3 entries
        assert_eq!(entries[0].session_id, Some("session-2".to_string()));
        assert_eq!(entries[1].session_id, Some("session-3".to_string()));
        assert_eq!(entries[2].session_id, Some("session-4".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_stats() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let logger = HookLogger::new(Some(temp_dir.path().to_path_buf()))?;

        // Log different types of events
        let event1 = ProjectLintEvent {
            event_type: EventType::PreToolUse,
            session_id: Some("session-1".to_string()),
            timestamp: None,
            cwd: None,
            context: EventContext {
                ide_source: "windsurf".to_string(),
                tool_name: Some("bash".to_string()),
                ..Default::default()
            },
        };

        let event2 = ProjectLintEvent {
            event_type: EventType::PostToolUse,
            session_id: Some("session-2".to_string()),
            timestamp: None,
            cwd: None,
            context: EventContext {
                ide_source: "claude".to_string(),
                tool_name: Some("node".to_string()),
                ..Default::default()
            },
        };

        logger.log_event(&event1, "Allow", None, Some(10))?;
        logger.log_event(&event2, "Warn", None, Some(20))?;
        logger.log_event(&event1, "Deny", None, Some(30))?;

        // Get stats
        let stats = logger.get_stats()?;

        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.event_counts.get("PreToolUse"), Some(&2));
        assert_eq!(stats.event_counts.get("PostToolUse"), Some(&1));
        assert_eq!(stats.source_counts.get("windsurf"), Some(&2));
        assert_eq!(stats.source_counts.get("claude"), Some(&1));
        assert_eq!(stats.decision_counts.get("Allow"), Some(&1));
        assert_eq!(stats.decision_counts.get("Warn"), Some(&1));
        assert_eq!(stats.decision_counts.get("Deny"), Some(&1));
        assert_eq!(stats.total_duration_ms, 60);
        assert_eq!(stats.event_count_with_duration, 3);
        assert_eq!(stats.min_duration_ms, 10);
        assert_eq!(stats.max_duration_ms, 30);

        Ok(())
    }

    #[tokio::test]
    async fn test_average_duration_calculation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let logger = HookLogger::new(Some(temp_dir.path().to_path_buf()))?;

        // Log events with different durations
        for i in 1..=3 {
            let event = ProjectLintEvent {
                event_type: EventType::PreToolUse,
                session_id: None,
                timestamp: None,
                cwd: None,
                context: EventContext {
                    ide_source: "test".to_string(),
                    ..Default::default()
                },
            };

            logger.log_event(&event, "Allow", None, Some(i * 10))?; // 10, 20, 30
        }

        let stats = logger.get_stats()?;
        assert_eq!(stats.average_duration_ms(), 20.0); // (10+20+30)/3

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_log_stats() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let logger = HookLogger::new(Some(temp_dir.path().to_path_buf()))?;

        let stats = logger.get_stats()?;

        assert_eq!(stats.total_events, 0);
        assert!(stats.event_counts.is_empty());
        assert!(stats.source_counts.is_empty());
        assert!(stats.decision_counts.is_empty());
        assert_eq!(stats.total_duration_ms, 0);
        assert_eq!(stats.event_count_with_duration, 0);
        assert_eq!(stats.min_duration_ms, 0);
        assert_eq!(stats.max_duration_ms, 0);
        assert_eq!(stats.average_duration_ms(), 0.0);

        Ok(())
    }
}
