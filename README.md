# Project Lint

A powerful CLI tool for maintaining clean project structure by warning about file placement, git branch issues, and automatically organizing files based on type.

## Features

- **Git Branch Validation**: Warn when creating files on inappropriate branches
- **File Organization**: Auto-detect and suggest proper file locations based on type
- **Script Location Checks**: Ensure scripts are in the correct directory (e.g., `bin/` vs `scripts/`)
- **Modular Rules**: Organize rules in separate TOML files for better maintainability
- **AST-Based Analysis**: Sophisticated code analysis using tree-sitter
- **Real-time Monitoring**: Watch for file changes and run checks automatically
- **Flexible Configuration**: TOML-based configuration with project-specific and global settings

## Installation

### From Source

```bash
git clone <repository-url>
cd project-lint
cargo build --release
cargo install --path .
```

### Using Cargo

```bash
cargo install project-lint
```

## Quick Start

1. **Initialize** the tool in your project:
   ```bash
   project-lint init
   ```

2. **Run linting** on your project:
   ```bash
   project-lint lint
   ```

3. **Watch** for changes in real-time:
   ```bash
   project-lint watch
   ```

## Configuration

Project-lint uses a hierarchical configuration system:

1. **Project-specific**: `.config/project-lint/config.toml` (highest priority)
2. **XDG Config**: `$XDG_CONFIG_HOME/project-lint/config.toml`
3. **Fallback**: `~/.config/project-lint/config.toml`

### Modular Rules System

Rules are organized in `.config/project-lint/rules/active/` where each `.toml` file represents a rule:

```
.config/project-lint/
├── config.toml                    # Main configuration
└── rules/
    └── active/                    # Active rules
        ├── git-branch-rules.toml  # Git branch validation
        ├── file-organization.toml  # File placement rules
        ├── script-location.toml   # Script directory rules
        ├── ast-analysis.toml      # AST-based code analysis
        └── custom-rules.toml      # Project-specific rules
```

### AST-Based Analysis

Project-lint now includes sophisticated AST-based analysis using tree-sitter:

#### Supported Languages
- **Rust**: Detect `println!`, `TODO` comments, unsafe blocks
- **Python**: Detect `print` statements, `TODO` comments, bare excepts
- **JavaScript/TypeScript**: Detect `console.log`, `TODO` comments, `alert()`
- **JSON/YAML/TOML**: Configuration file validation

#### AST Analysis Features
```toml
[ast_analysis]
enabled = true
max_file_size_mb = 10
timeout_seconds = 30

[supported_languages]
rust = true
python = true
javascript = true
typescript = true
```

#### Example AST Issues
```
⚠️  src/main.rs:15:5 - Remove debug println! statement before committing (no_debug_prints)
📝 src/utils.rs:23:1 - TODO comment found - consider addressing (todo_comment)
🔍 src/app.js:42:8 - Remove debug console.log statement before committing (no_debug_prints)
```

### Example Rule Files

#### Git Branch Rules (`git-branch-rules.toml`)
```toml
name = "git-branch-rules"
description = "Validate git branch usage and warn about inappropriate branches"
enabled = true
severity = "warning"

[git]
warn_wrong_branch = true
allowed_branches = ["main", "master", "feature/*", "fix/*"]
forbidden_branches = ["develop", "staging", "wip/*"]

[conditions]
require_git_repo = true

[messages]
branch_forbidden = "⚠️  Working on branch '{branch}' which is forbidden for file creation"
branch_not_allowed = "⚠️  Working on branch '{branch}' which may not be appropriate for file creation"
```

#### File Organization Rules (`file-organization.toml`)
```toml
name = "file-organization"
description = "Ensure files are placed in appropriate directories based on type"
enabled = true
severity = "warning"

[file_mappings]
"*.sh" = "bin/"
"*.py" = "scripts/"
"*.js" = "scripts/"
"*.ts" = "scripts/"
"*.md" = "docs/"
"*.rs" = "src/"

[ignored_patterns]
"node_modules/" = true
".git/" = true
"target/" = true

[messages]
file_misplaced = "📁 File '{file}' should be in '{target_dir}' directory (matches pattern '{pattern}')"
```

#### AST Analysis Rules (`ast-analysis.toml`)
```toml
name = "ast-analysis"
description = "AST-based code analysis using tree-sitter"
enabled = true
severity = "warning"

[ast_analysis]
enabled = true
max_file_size_mb = 10
timeout_seconds = 30

[rust_rules]
check_println = true
check_todo_comments = true
check_unsafe_blocks = true

[python_rules]
check_print_statements = true
check_todo_comments = true
check_bare_except = true

[javascript_rules]
check_console_log = true
check_todo_comments = true
check_alert_statements = true

[messages]
println_detected = "🔍 Debug println! statement found at {file}:{line}:{column}"
todo_comment = "📝 TODO comment found at {file}:{line}:{column}"
console_log = "🔍 Debug console.log found at {file}:{line}:{column}"
```

### Legacy Configuration

For backward compatibility, you can still use the old configuration format:

```toml
[git]
warn_wrong_branch = true
allowed_branches = ["main", "master", "feature/*"]
forbidden_branches = ["develop", "staging"]

[files]
auto_move = true
type_mappings = { "*.sh" = "bin/", "*.py" = "scripts/" }
ignored_patterns = ["node_modules/", ".git/"]

[directories]
warn_scripts_location = true
scripts_directory = "bin"
structure = { "src/" = ["*.rs", "*.py"] }

[rules]
enabled_checks = ["git_branch", "file_location", "directory_structure"]
custom_rules = [
    { name = "no_tmp", pattern = "*.tmp", message = "No temp files", severity = "error" }
]
```

## Commands

### `init`

Initialize project-lint configuration.

```bash
project-lint init [--force]
```

Options:
- `--force`: Overwrite existing configuration

### `lint`

Run linting checks on the project.

```bash
project-lint lint [--path <PATH>]
```

Options:
- `--path`: Path to the project root (defaults to current directory)

### `watch`

Watch for file changes and run linting automatically.

```bash
project-lint watch [--path <PATH>]
```

Options:
- `--path`: Path to the project root (defaults to current directory)

## Examples

### Basic Usage

```bash
# Initialize in a new project
cd my-project
project-lint init

# Run checks
project-lint lint

# Watch for changes
project-lint watch
```

### AST Analysis Examples

```bash
# Run with AST analysis (automatically included)
project-lint lint

# Example output:
# ⚠️  src/main.rs:15:5 - Remove debug println! statement before committing (no_debug_prints)
# 📝 src/utils.rs:23:1 - TODO comment found - consider addressing (todo_comment)
# 🔍 src/app.js:42:8 - Remove debug console.log statement before committing (no_debug_prints)
```

### Managing Rules

```bash
# Enable a rule
echo 'enabled = true' > .config/project-lint/rules/active/my-rule.toml

# Disable a rule
echo 'enabled = false' > .config/project-lint/rules/active/my-rule.toml

# Create a new custom rule
cat > .config/project-lint/rules/active/my-custom-rule.toml << EOF
name = "my-custom-rule"
description = "My custom project rule"
enabled = true
severity = "warning"

[[rules]]
name = "no_debug_files"
pattern = "debug.*"
message = "Debug files should not be committed"
severity = "warning"
EOF
```

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running with Verbose Logging

```bash
project-lint --verbose lint
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details. 