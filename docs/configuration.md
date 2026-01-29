# Configuration Guide

This guide covers how to configure project-lint using both the TUI interface and manual configuration files.

## Configuration Overview

Project-lint uses a hierarchical configuration system:

1. **Global Config**: `~/.config/project-lint/config.toml`
2. **Project Config**: `.config/project-lint/config.toml`  
3. **Local Config**: `project-lint.toml`

Configuration is loaded in order, with later configs overriding earlier ones.

## TUI Configuration

### Launch TUI
```bash
project-lint configure
```

### TUI Navigation

#### Main Sections
- **Rules Tab (1)**: Manage modular rules and custom rules
- **Profiles Tab (2)**: View and manage active profiles
- **Settings Tab (3)**: Configure global settings

#### Rules Management
- **Navigate**: `↑/↓` or `j/k`
- **Toggle Rule**: `Space` or `Enter`
- **Save Changes**: `s`
- **Quit**: `q`

#### Rule Details Panel
When a rule is selected, the right panel shows:
- Rule name and description
- Current status (enabled/disabled)
- Severity level
- Trigger events
- Associated custom rules

## Manual Configuration

### Config File Structure

```toml
[git]
enabled = true
default_branch = "main"
allowed_branches = ["main", "develop"]
forbidden_branches = ["master"]

[files]
enabled = true
case_sensitive = false
max_filename_length = 255
forbidden_patterns = ["temp", "backup"]

[directories]
enabled = true
enforce_structure = true
auto_create = false

[rules]
custom_rules = []
```

### Modular Rules

```toml
[[modular_rules]]
name = "file-naming-enforcer"
description = "Enforces consistent file naming conventions"
enabled = true
severity = "warning"
triggers = ["pre_write_code", "post_read_code"]

[modular_rules.file_mappings]
"*.tmp" = "temp/"
"*.bak" = "backups/"
"README.md" = "./"
```

### Custom Rules

```toml
[[rules.custom_rules]]
name = "no-hardcoded-secrets"
pattern = "*"
triggers = ["pre_write_code"]
check_content = true
content_pattern = "password|secret|token"
severity = "error"
message = "Potential hardcoded secret detected"
condition = "contains"
```

## Rule Categories

### 1. File Naming Rules
- **Purpose**: Enforce consistent file naming conventions
- **Triggers**: `pre_write_code`, `post_read_code`
- **Config**: `file_naming.toml`

### 2. Package Organization Rules  
- **Purpose**: Ensure proper package/directory structure
- **Triggers**: `post_read_code`
- **Config**: `package_organization.toml`

### 3. Security Rules
- **Purpose**: Detect security issues and vulnerabilities
- **Triggers**: `pre_write_code`, `post_read_code`
- **Config**: `security.toml`

### 4. Git Rules
- **Purpose**: Enforce git branch and commit standards
- **Triggers**: `pre_commit`, `pre_push`
- **Config**: `git.toml`

### 5. TypeScript Rules
- **Purpose**: TypeScript-specific linting and best practices
- **Triggers**: `pre_write_code`, `post_read_code`
- **Config**: `typescript.toml`

### 6. AST Rules
- **Purpose**: Abstract syntax tree analysis for code patterns
- **Triggers**: `post_read_code`
- **Config**: `ast.toml`

### 7. Runtime Guard Rules
- **Purpose**: Detect runtime issues and browser compatibility
- **Triggers**: `pre_write_code`, `post_read_code`
- **Config**: `runtime_guards.toml`

## Rule Triggers

Available trigger events:
- `session_start`, `session_end`
- `pre_tool_use`, `post_tool_use`
- `pre_read_code`, `post_read_code`
- `pre_write_code`, `post_write_code`
- `pre_run_command`, `post_run_command`
- `pre_user_prompt`, `post_model_response`
- `notification`, `permission_request`
- `stop`, `subagent_stop`

## Severity Levels

- **Error**: Blocks execution (exit code 2)
- **Warning**: Shows warning but allows execution
- **Info**: Informational message only

## Rule Conditions

### Content Checking
```toml
check_content = true
content_pattern = "pattern"
condition = "contains"  # or "must_contain"
```

### Path-based Rules
```toml
pattern = "*.rs"
required_if_path_exists = "Cargo.toml"
```

### Exception Patterns
```toml
exception_pattern = "test_*.tmp"
```

## Profiles

### Using Profiles
```toml
[profiles]
default = "web-development"
available = ["web-development", "data-science", "mobile"]

[[profiles.list]]
name = "web-development"
description = "Rules for web development projects"
rules = ["typescript", "file-naming", "security"]
```

### Profile Activation
```bash
# Activate specific profile
project-lint --profile web-development

# List available profiles
project-lint --list-profiles
```

## Environment Variables

```bash
# Log level
export RUST_LOG=debug

# Config directory
export PROJECT_LINT_CONFIG_DIR=/path/to/config

# Log directory
export PROJECT_LINT_LOG_DIR=/path/to/logs
```

## Examples

### Basic Configuration
```toml
[git]
enabled = true
default_branch = "main"

[files]
enabled = true
case_sensitive = false

[[rules.custom_rules]]
name = "no-temp-files"
pattern = "*.tmp"
severity = "warning"
message = "Temporary files should not be committed"
```

### Advanced Configuration
```toml
[[modular_rules]]
name = "typescript-strict"
description = "Strict TypeScript rules"
enabled = true
severity = "error"
triggers = ["pre_write_code", "post_read_code"]

[[modular_rules.rules]]
name = "no-any-types"
pattern = "*.ts"
check_content = true
content_pattern = ": any"
severity = "error"
message = "Avoid using 'any' type"
```

## Troubleshooting

### Configuration Not Loading
- Check file permissions
- Verify TOML syntax: `toml-lint config.toml`
- Check config path with `project-lint --config-path`

### Rules Not Triggering
- Verify trigger events match your workflow
- Check rule patterns match target files
- Enable debug logging: `RUST_LOG=debug project-lint`

### TUI Issues
- Ensure terminal supports UTF-8
- Check terminal size (minimum 80x24)
- Use `--no-tui` flag for CLI-only mode
