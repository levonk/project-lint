# Agent Directives for Project-Lint

Project-lint is a **modular, multi-faceted linting and organization tool** written in Rust. It helps maintain clean project structures by enforcing naming conventions, validating package organization, performing security scans, and conducting AST-based code analysis.

---

## Understanding Project-Lint Architecture

### 1. Core Engine

- **`src/main.rs`**: Entry point using `clap` for CLI handling.
- **`src/config.rs`**: The heart of the tool. Manages configuration loading from `.config/project-lint/`, profile merging, and modular rule processing.
- **`src/lib.rs`**: Public API and module definitions.

### 2. Analysis Modules

- **`src/file_naming.rs`**: Validates standard filenames (e.g., `.devcontainer`, `package.json`). Uses fuzzy Levenshtein distance to detect typos and supports auto-renaming.
- **`src/package_organization.rs`**: Enforces path structures for monorepos (e.g., `packages/{category}/{platform}/...`).
- **`src/ast.rs`**: Uses `tree-sitter` for deep code analysis across multiple languages (Rust, Python, TS, etc.).
- **`src/detection.rs`**: Generic regex-based pattern matching and replacement engine.
- **`src/security.rs`**: Scans for common security pitfalls and hardcoded secrets.
- **`src/typescript.rs`**: Specialized linter for TypeScript/JavaScript projects.

### 3. Lifecycle & Commands

- **`src/commands/lint.rs`**: The primary execution flow. It orchestrates all scanners and applies auto-fixes.
- **`src/commands/watch.rs`**: Uses `notify` to provide a reactive linting experience.
- **`src/commands/hook.rs`**: Handles IDE events (PreToolUse, etc.) via stdin/stdout.
- **`src/hooks/`**: Event-driven architecture for IDE integration.
  - **`src/hooks/mod.rs`**: Unified event model (`ProjectLintEvent`).
  - **`src/hooks/mappers/`**: IDE-specific payload translators (Windsurf, Claude, Kiro).
  - **`src/hooks/engine.rs`**: Rule evaluation logic for events.
- **`src/profiles.rs`**: Handles dynamic profile activation based on project contents and events.

---

## IDE Hook Integration

Project-lint can be used as a hook handler for various IDEs and agents (Claude Code, Windsurf, Kiro).

### Usage

```bash
# For Windsurf (Cascade Hooks)
project-lint hook --source windsurf

# For Claude Code
project-lint hook --source claude
```

### Supported Events

- `session_start`, `session_end`
- `pre_tool_use`, `post_tool_use`
- `pre_read_code`, `post_read_code`
- `pre_write_code`, `post_write_code`
- `pre_run_command`, `post_run_command`
- `pre_user_prompt`, `post_model_response`
- `notification`, `permission_request`
- `stop`, `subagent_stop`

### Event-Driven Rules

Rules can now be restricted to specific events by adding a `triggers` field:

```toml
[[rules.custom_rules]]
name = "no-dangerous-commands"
pattern = "*"
triggers = ["pre_run_command"]
check_content = true
content_pattern = "rm -rf /"
severity = "error"
message = "Blocking dangerous command execution."
```

---

## Development Standards

### Configuration Discovery

Project-lint looks for configuration in two main places:

1. **Project-local**: `<project-root>/.config/project-lint/`
2. **User-global**: `~/.config/project-lint/` (or platform equivalent)

### Profiles vs. Modular Rules

- **Profiles** (`rules/profiles/*.toml`): High-level collections of checks enabled for specific project types.
- **Modular Rules** (`rules/active/*.toml`): Granular, file-specific rules that can be shared across projects.

### Auto-Fixing

Scanners should ideally implement `apply_fixes` method. The `lint` command handles the `--fix` and `--dry-run` logic centrally.

---

## Development Standards

- **Rust Version**: 2021 Edition.
- **Logging**: Use the `tracing` crate. Prefer `debug!` for internal flow and `info!` for user-facing status.
- **Errors**: Use `anyhow` for top-level errors and `thiserror` for custom error types in `src/utils.rs`.
- **Testing**:
  - Unit tests live in the same file as the code (e.g., `src/file_naming.rs` has its own `mod tests`).
  - Use `tempfile` and `assert_fs` for filesystem-related tests.
  - Run tests with `cargo test`.

---

## Common Tasks for Agents

- This project uses a CLI ticket system for task management. Run `tk help` when you need to use it.

### Adding a New Scanner

1. Create `src/your_scanner.rs`.
2. Implement a scanner struct and a `scan` method.
3. Register the module in `src/lib.rs` and `src/main.rs`.
4. Integrate into `src/commands/lint.rs` within the `run` function.
5. Add a toggle in `src/config.rs` (optional but recommended).

### Modifying the Naming Dictionary

Update the `exact_mismatches` and `expected_names` in `src/file_naming.rs` to include new standard files or common mistakes.

### Implementing New AST Rules

Update the `ASTAnalyzer` in `src/ast.rs` and define new `tree-sitter` queries.
