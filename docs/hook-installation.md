# Hook Installation Guide

Project-lint can automatically install hooks for various AI coding agents to intercept and validate tool execution.

## Supported Agents

- **Windsurf** - `.windsurf/` directory
- **Claude Code** - `.claude/` directory (installs `hook.sh` **and** registers it in `.claude/settings.json`)
- **Cursor** - `.cursor/` directory
- **Generic** - `hooks/` directory (for custom setups)
- **Devin CLI** - `.devin/hooks.v1.json`
- **Pi** - `.pi/extensions/project-lint-hook.ts`
- **Git hooks** - `.git/hooks/pre-commit` and `pre-push` (with worktree isolation gate)
- **All** - installs **both** git hooks and Claude Code hooks in one command
- **GitHub / GitLab** - CI workflow files

## Installation Commands

### Install for Specific Agent

```bash
# Install for Windsurf
project-lint install-hook --agent windsurf

# Install for Claude Code (creates hook.sh + settings.json)
project-lint install-hook --agent claude

# Install for Cursor
project-lint install-hook --agent cursor

# Install for generic/custom setup
project-lint install-hook --agent generic

# Install git hooks (pre-commit/pre-push with worktree isolation)
project-lint install-hook --agent git-hooks

# Install BOTH git hooks and Claude Code hooks at once
project-lint install-hook --agent all
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

For Claude Code, also creates/merges `.claude/settings.json` registering the
hook for `PreToolUse` (matching `Edit|Write|MultiEdit|NotebookEdit|Task`) and
`Stop`. The merge is non-destructive — existing permissions and hook entries
are preserved:
```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Edit|Write|MultiEdit|NotebookEdit|Task",
        "hooks": [ { "type": "command", "command": ".claude/hook.sh" } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command", "command": ".claude/hook.sh" } ] }
    ]
  }
}
```

### Git Hooks (pre-commit / pre-push)
`--agent git-hooks` (or `all`) installs `.git/hooks/pre-commit` and `pre-push`.
The pre-commit hook includes a **worktree isolation gate** that blocks commits
to protected branches (`main`, `master`, `trunk`, `develop`) when not inside a
linked `git worktree add` worktree. Work on protected branches must happen in a
worktree so the main worktree stays clean.

## Worktree Isolation

The `worktree-isolation-enforcer` rule (configured in
`.config/project-lint/rules/active/worktree-isolation.toml`) enforces that all
work on protected branches happens inside a linked git worktree. It acts at
three points:

1. **Pre-commit hook** (`--agent git-hooks`/`all`): blocks commits to a
   protected branch outside a linked worktree.
2. **PreToolUse** (Edit/Write/MultiEdit/NotebookEdit/Task): blocks **direct
   edits** and **subagent dispatch** on a protected branch in the main
   worktree. This closes the gap where only subagent dispatch was blocked.
   Write-tool blocking is **scoped by `protected_paths`** (default
   `["src/**"]`): only writes matching one of the globs are blocked, so
   edits to `docs/`, config, `README.md`, etc. on main are allowed while
   source-code edits are not. Subagent dispatch is never scoped by paths —
   it is always blocked on a protected branch in the main worktree.
3. **Stop**: blocks stopping with a dirty working tree on a protected branch
   in the main worktree. This fires on **every** Stop event — there is no
   once-per-session suppression, so a recovered-but-still-dirty state
   re-triggers the guard.

All three checks are no-ops inside a linked worktree and on non-protected
branches. Disable the rule by setting `enabled = false` in
`worktree-isolation.toml`, or tune which write paths trigger it via the
`protected_paths` glob list (empty defaults to `["src/**"]`), or add a
`disabled_if_path_exists` marker.

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
