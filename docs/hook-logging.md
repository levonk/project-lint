# Hook Logging Guide

Project-lint automatically logs all hook interactions with AI coding agents for debugging and analysis.

## Log Location

Logs are stored in:
- **Linux/macOS**: `~/.local/share/project-lint/logs/`
- **Windows**: `%LOCALAPPDATA%\project-lint\logs\`

Log files are named by date: `hook-log-YYYY-MM-DD.jsonl`

## Viewing Logs

### Recent Logs
```bash
# Show last 50 log entries
project-lint logs

# Show custom number of entries
project-lint logs --limit 100

# Show logs from specific directory
project-lint logs --dir /path/to/logs
```

### Statistics Only
```bash
# Show aggregated statistics
project-lint logs --stats
```

## Log Format

Each log entry is a JSON line with these fields:

```json
{
  "timestamp": "2025-01-28T20:00:00Z",
  "event_type": "PreToolUse",
  "source": "windsurf",
  "session_id": "session-123",
  "file_path": "/path/to/file.rs",
  "tool_name": "bash",
  "command": "npm install express",
  "decision": "Warn",
  "message": "This project uses pnpm...",
  "duration_ms": 15
}
```

## Log Output Example

```
📋 Recent Hook Logs (showing 50 entries)
================
⚠️ 20:00:15 [windsurf] PreToolUse
  Session: session-123
  Tool: bash
  Command: npm install express
  Message: 🚫 This project uses pnpm... Suggested: pnpm install express
  Duration: 15ms

✅ 20:00:20 [windsurf] PostToolUse
  Session: session-123
  Tool: bash
  Duration: 5ms
```

## Statistics Output

```
📊 Hook Statistics
================
Total events: 150
Average duration: 12.34ms
Min duration: 2ms
Max duration: 150ms

📈 Events by Type:
  PreToolUse: 75
  PostToolUse: 75

🔌 Events by Source:
  windsurf: 120
  claude: 30

⚖️  Events by Decision:
  Allow: 140
  Warn: 8
  Deny: 2
```

## Configuring Logging

### Log Level
Set via environment variable:
```bash
export RUST_LOG=debug  # More verbose logging
export RUST_LOG=info   # Standard logging
export RUST_LOG=warn   # Only warnings and errors
```

### Log Directory
Customize log location:
```bash
project-lint hook --source windsurf --log-dir /custom/log/path
```

## Log Analysis

### Finding Issues
```bash
# Look for denied events
grep '"decision":"Deny"' ~/.local/share/project-lint/logs/hook-log-*.jsonl

# Find slow operations
jq 'select(.duration_ms > 100)' ~/.local/share/project-lint/logs/hook-log-*.jsonl

# Count by decision type
jq -r '.decision' ~/.local/share/project-lint/logs/hook-log-*.jsonl | sort | uniq -c
```

### Performance Monitoring
```bash
# Average duration by event type
jq -s 'group_by(.event_type) | map({event_type: .[0].event_type, avg_duration: (. | map(.duration_ms) | add / length)})' ~/.local/share/project-lint/logs/hook-log-*.jsonl

# Find longest operations
jq -s 'sort_by(.duration_ms) | reverse | .[0:10]' ~/.local/share/project-lint/logs/hook-log-*.jsonl
```

## Log Retention

- Log files are created daily
- No automatic cleanup - manage manually
- Consider adding logrotate for production use

Example logrotate config:
```
~/.local/share/project-lint/logs/hook-log-*.jsonl {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
}
```

## Troubleshooting

### No Logs Appearing
- Check log directory permissions
- Verify logging is enabled (RUST_LOG env var)
- Ensure hooks are properly installed

### Logs Not Detailed Enough
- Set `RUST_LOG=debug` for verbose output
- Check if rules are configured to trigger
- Verify IDE is sending events to hooks

### Large Log Files
- Implement log rotation
- Use `--stats` for summary instead of raw logs
- Consider filtering by date range

## Integration with Monitoring

### Export to Metrics
```bash
# Convert logs to Prometheus format
project-lint logs --stats | jq '
  {
    total_events: .total_events,
    avg_duration_ms: (.total_duration_ms / .event_count_with_duration),
    denied_events: .decision_counts.Deny // 0
  }
'
```

### Alert on Issues
```bash
# Alert if too many denied events
denied_count=$(jq -r '.decision_counts.Deny // 0' <(project-lint logs --stats))
if [ "$denied_count" -gt 10 ]; then
    echo "Alert: $denied_count denied events detected"
fi
```
