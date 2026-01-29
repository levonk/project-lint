# Pnpm Workspace Enforcement

This feature automatically detects pnpm workspaces and enforces the use of `pnpm` commands instead of `npm` commands.

## How It Works

1. **Detection**: The rule checks if the current project is a pnpm workspace by looking for:

   - `packageManager` field in `package.json` starting with `pnpm`
   - Presence of `pnpm-workspace.yaml` or `pnpm-workspace.yml`
   - Presence of `pnpm-lock.yaml`

2. **Interception**: On `PreToolUse` events, it intercepts commands that start with `npm `

3. **Response**: Shows a helpful message and rewrites the command to use `pnpm` instead

## Configuration

Add to your project-lint configuration (`~/.config/project-lint/rules/active/pnpm-enforcer.toml`):

```toml
[[rules.custom_rules]]
name = "pnpm-workspace-enforcer"
pattern = "*"
triggers = ["pre_tool_use"]
check_content = false
severity = "warning"
message = "This project uses pnpm. Please use 'pnpm' instead of 'npm' commands."
```

## IDE Integration

### Windsurf

```bash
project-lint hook --source windsurf
```

The hook will:
- Detect npm commands in tool execution
- Show a warning message with the suggested pnpm command
- Automatically rewrite the command input if supported

### Claude Code

```bash
project-lint hook --source claude
```

### Other IDEs

```bash
project-lint hook --source generic
```

## Example

When an AI tries to run:

```bash
npm install express
```

The hook will:

1. Detect this is a pnpm workspace
2. Show: "🚫 This project uses pnpm... Found: npm install express Suggested: pnpm install express"
3. Automatically rewrite the command to: `pnpm install express`

## Testing

```bash
cd /home/micro/p/gh/levonk/project-lint
cargo test pnpm_enforcement
```

## Benefits

- **Consistency**: Ensures all developers use the same package manager
- **Prevents Errors**: Avoids mixing npm and pnpm which can cause lockfile conflicts
- **Educational**: Helps AIs learn the correct package manager for each project
- **Automatic**: No manual intervention required - commands are rewritten automatically
