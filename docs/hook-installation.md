# Hook Installation Guide

Project-lint can automatically install hooks for various AI coding agents to intercept and validate tool execution.

## Supported Agents

- **Windsurf** - `.windsurf/` directory
- **Claude Code** - `.claude/` directory  
- **Cursor** - `.cursor/` directory
- **Generic** - `hooks/` directory (for custom setups)

## Installation Commands

### Install for Specific Agent

```bash
# Install for Windsurf
project-lint install-hook --agent windsurf

# Install for Claude Code
project-lint install-hook --agent claude

# Install for Cursor
project-lint install-hook --agent cursor

# Install for generic/custom setup
project-lint install-hook --agent generic
```

### Custom Installation Directory

```bash
# Install to custom directory
project-lint install-hook --agent windsurf --dir /path/to/hooks

# Force overwrite existing hooks
project-lint install-hook --agent claude --force
```

## What Gets Installed

### Hook Script
Each installation creates a `hook.sh` script that:
- Reads event data from stdin
- Passes it to `project-lint hook --source <agent>`
- Exits with the same code as project-lint

### Configuration Files
For Windsurf, also creates `config.toml` with hook mappings:
```toml
[hooks]
pre_tool_use = "./hook.sh"
post_tool_use = "./hook.sh"
pre_read_code = "./hook.sh"
post_read_code = "./hook.sh"
pre_write_code = "./hook.sh"
post_write_code = "./hook.sh"
```

## Hook Events

The hooks intercept these events:
- **PreToolUse** - Before tool execution
- **PostToolUse** - After tool execution
- **PreReadCode** - Before reading files
- **PostReadCode** - After reading files
- **PreWriteCode** - Before writing files
- **PostWriteCode** - After writing files

## Manual Hook Setup

If you prefer manual setup, create a hook script:

```bash
#!/bin/bash
PROJECT_LINT_BIN="path/to/project-lint"
HOOK_TYPE="your-agent"

EVENT_DATA=$(cat)
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?
exit $EXIT_CODE
```

## IDE Integration

### Windsurf Integration
1. Run: `project-lint install-hook --agent windsurf`
2. Windsurf will automatically use the hooks for configured events

### Claude Code Integration  
1. Run: `project-lint install-hook --agent claude`
2. Configure Claude Code to use `.claude/hook.sh`

### Cursor Integration
1. Run: `project-lint install-hook --agent cursor`
2. Configure Cursor to use `.cursor/hook.sh`

## Troubleshooting

### Hook Not Executing
- Ensure hook script is executable: `chmod +x hook.sh`
- Check IDE configuration for hook paths
- Verify `project-lint` is in PATH or use absolute path

### Permission Denied
- Run with `--force` to overwrite existing hooks
- Check directory permissions
- Ensure `project-lint` has execute permissions

### Hook Not Found
- Verify installation directory exists
- Check that `project-lint` binary exists at expected path
- Use absolute path in hook script if needed

## Example Usage

```bash
# Install hook for Windsurf in current project
project-lint install-hook --agent windsurf

# Now when Windsurf executes tools, project-lint will:
# 1. Intercept the tool execution
# 2. Check against configured rules
# 3. Allow, warn, or block based on rule evaluation
# 4. Log the interaction for debugging
```
