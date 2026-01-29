use crate::hooks::{logger::{HookLogger, HookLogEntry}, ProjectLintEvent, EventType, EventContext};
use crate::utils::Result;
use tempfile::TempDir;
use std::path::PathBuf;
use chrono::Utc;

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
