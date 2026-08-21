#!/bin/bash
# Claude Code hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

# Resolve project-lint binary at runtime (no hardcoded paths)
# Checks dev builds relative to known project-root env vars, then PATH
PROJECT_LINT_BIN="project-lint"
for _root in "${CLAUDE_PROJECT_DIR:-}" "${CURSOR_PROJECT_DIR:-}" "${DEVIN_PROJECT_DIR:-}" "${PWD:-}"; do
  if [ -n "$_root" ] && [ -x "$_root/target/release/project-lint" ]; then
    PROJECT_LINT_BIN="$_root/target/release/project-lint"
    break
  fi
  if [ -n "$_root" ] && [ -x "$_root/target/debug/project-lint" ]; then
    PROJECT_LINT_BIN="$_root/target/debug/project-lint"
    break
  fi
done
HOOK_TYPE="claude"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
