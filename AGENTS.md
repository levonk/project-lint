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
- **`project-lint-core/src/scanners/dockerfile_lint.rs`**: Lints `Dockerfile` / `Dockerfile.*` / `*.dockerfile` files — pinned image digests, no `COPY .`, non-root `USER`, no `:latest` tags, `HEALTHCHECK` presence, `apk add --no-cache`, `apt-get install --no-install-recommends` with cleanup, multi-stage build detection, `.dockerignore` presence, and digest-pinning exemptions for `scratch` / distroless images. Gated by the `dockerfile_lint` check name.
- **`project-lint-core/src/scanners/compose_lint.rs`**: Lints `docker-compose*.yml` / `compose*.yml` files — pinned image digests, no `:latest` / floating tags, no privileged mode, `security_opt: ["no-new-privileges:true"]`, no `/var/run/docker.sock` mounts (exempt by proxy label), non-root `user`, no `network_mode: host` / `pid: host`, `cap_drop: ["ALL"]`, `read_only: true`, `healthcheck`, restart policies, resource limits, safe port bindings (no `0.0.0.0`), and watchtower/wud update labels in ops mode. Uses `serde_yaml` for parsing. Gated by the `compose_lint` check name.
- **`project-lint-core/src/scanners/skill_markdown.rs`**: Validates `SKILL.md` files in the skills-src wrapper pattern — body line limit (default 80, configurable via `[scanner_config.skill_markdown]`), `scripts/refresh.sh` presence, and required frontmatter fields (`name`, `description`, `version`). Gated by the `skill_markdown` check name.
- **`project-lint-core/src/scanners/git_sync.rs`**: Warns when the local repo is out of sync with its remote — runs `git fetch` (toggleable via `[scanner_config.git_sync] fetch_before_compare`), then checks the main branch and the current branch against their upstreams for behind / ahead / diverged states, plus a dirty-working-tree warning. Gated by the `git_sync` check name.
- **`project-lint-core/src/scanners/nix_flake.rs`**: Validates `flake.nix` (inputs have URLs, inputs pinned, outputs is a function, has description, no spurious `flake = false`) and `flake.lock` (present, fresh, every node has narHash). Parses `flake.nix` with regex and `flake.lock` as JSON. Gated by the `nix_flake` check name.
- **`project-lint-core/src/scanners/devbox_json.rs`**: Validates `devbox.json` as JSON — name present, packages is an object (not array), `$schema` present, `devbox.lock` present, GitHub packages pinned, init_hook not empty, scripts delegate to `just`, no `npx`/`bunx`/`yarn`. Gated by the `devbox_json` check name.
- **`project-lint-core/src/scanners/nix_shell.rs`**: Validates `shell.nix` and `default.nix` — uses `pkgs.mkShell`, has `buildInputs`/`packages`, no floating `import <nixpkgs>`, and `default.nix` is not a shell definition. Gated by the `nix_shell` check name.
- **`project-lint-core/src/scanners/envrc_content.rs`**: Validates `.envrc` files — no hardcoded secrets, uses `use devbox`/`use flake`, no `direnv allow`, `watch_file devbox.json` when using devbox, no hardcoded absolute paths. Gated by the `envrc_content` check name.
- **`project-lint-core/src/scanners/github_workflow.rs`**: Validates `.github/workflows/*.yml` files for security and quality — explicit `permissions:` block, minimal token scope (`contents: read`), no `pull_request_target`, SHA-pinned actions, no secret env injection, no `sudo`, valid `runs-on`, `concurrency`, `timeout-minutes`, and devbox usage. Configurable via `[scanner_config.github_workflow]`. Gated by the `github_workflow` check name.
- **`project-lint-core/src/scanners/dependabot.rs`**: Validates `.github/dependabot.yml` — ecosystem coverage, schedule intervals, group config, assignees/reviewers, and `github-actions` ecosystem presence when workflows exist. Configurable via `[scanner_config.dependabot]`. Gated by the `dependabot` check name.
- **`project-lint-core/src/scanners/justfile_content.rs`**: Validates `justfile` / `Justfile` content — required targets (`quality`, `quality-full`, `ci`, `bootstrap`), devbox wrapper usage, no absolute paths, no forbidden commands (`npx`, `bunx`, `yarn`), and no raw `cargo` calls in devbox projects. Configurable via `[scanner_config.justfile_content]`. Gated by the `justfile_content` check name.
- **`project-lint-core/src/scanners/makefile_content.rs`**: Validates `Makefile` content — flags Makefile presence (should migrate to justfile), no absolute paths, no `cd /absolute`, and just-delegation pattern. Configurable via `[scanner_config.makefile_content]`. Gated by the `makefile_content` check name.
- **`project-lint-core/src/scanners/process_compose.rs`**: Validates `process-compose.yaml` / `.yml` — valid `command` fields, health checks, restart policies, no absolute paths, and devbox usage. Configurable via `[scanner_config.process_compose]`. Gated by the `process_compose` check name.
- **`project-lint-core/src/scanners/nx_config.rs`**: Validates `nx.json` for Nx monorepo configuration — checks for `namedInputs` (cache reuse), `targetDefaults` (standard targets), `defaultBase` (main branch), and cacheable operations. Configurable via `[scanner_config.nx_config]` with `require_named_inputs` and `require_target_defaults`. Gated by the `nx_config` check name.
- **`project-lint-core/src/scanners/nx_project.rs`**: Validates Nx `project.json` files across a monorepo — checks that project `name` matches its directory, targets are defined, and `tags` are present for dependency boundary enforcement. Uses the centralized exclusion list to skip `node_modules/`. Configurable via `[scanner_config.nx_project]` with `require_name_matches_dir` and `require_tags`. Gated by the `nx_project` check name.
- **`project-lint-core/src/scanners/pnpm_workspace.rs`**: Validates `pnpm-workspace.yaml` content — checks for `packages:` field with glob patterns, verifies globs match at least one directory, detects `node_modules` in package globs, and optionally validates `catalog:` section. Configurable via `[scanner_config.pnpm_workspace]` with `require_catalog` and `check_glob_matches`. Gated by the `pnpm_workspace` check name.
- **`project-lint-core/src/scanners/node_modules_integrity.rs`**: Detects corrupted pnpm `node_modules/` structures — checks for `.pnpm/` directory, verifies top-level packages are symlinks (not real directories), validates `.modules.yaml` has `packageManager: pnpm`, detects foreign lockfiles, and checks root `package.json` has `packageManager` field. Only activates when `pnpm-lock.yaml` exists. Configurable via `[scanner_config.node_modules_integrity]`. Gated by the `node_modules_integrity` check name.
- **`project-lint-core/src/scanners/config_validation.rs`**: Validates configuration files (`tsconfig.json`, `eslint.config.*`, `tailwind.config.*`, `package.json`) for best-practice settings — strict mode, module resolution, path aliases, required base configs, content arrays, and package type/exports fields. Configurable via `[scanner_config.config_validation]` (required_eslint_base, require_type_module, check_tailwind). Gated by the `config_validation` check name.
- **`project-lint-core/src/scanners/markdown_frontmatter.rs`**: Validates YAML frontmatter in all `.md` files — required fields (title, synopsis, tags), frontmatter delimiters, and ADR-specific rules (adr-id format, status, date, version) for files under `internal-docs/adr` or `docs-internal/adr`. Configurable via `[scanner_config.markdown_frontmatter]` (require_frontmatter, adr_dirs). Gated by the `markdown_frontmatter` check name.
- **`project-lint-core/src/scanners/runtime_guards.rs`**: Detects unguarded browser API access (`window`, `document`, `navigator`, `localStorage`, `sessionStorage`) and `typeof window/document` patterns in TypeScript/JavaScript files, requiring the runtime-guards package import. Configurable via `[scanner_config.runtime_guards]` (guards_package, check_extensions). Gated by the `runtime_guards` check name.
- **`project-lint-core/src/utils.rs` (centralized exclusion list)**: Shared utility that all WalkDir-based scanners use to skip build artifacts, dependency directories, and VCS internals (`node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `.turbo/`, `.nuxt/`, `.svelte-kit/`, `.git/`, `.devbox/gen/`, `.cache/`, `coverage/`, and configurable `vendor/`). Configurable via `[scanner_config.exclusion]` with `extra_excludes`, `allow_vendor`, and `max_depth`. Provides `DEFAULT_EXCLUDED_DIRS`, `build_exclusions()`, `is_excluded_rel()`, `is_excluded_entry()`, and `walk_project()`. All WalkDir-based scanners (`rust_conventions`, `dockerfile_lint`, `compose_lint`, `skill_markdown`, `magic_numbers`, `vault_security`, `file_naming`, `nix_flake`, `devbox_json`, `nix_shell`, `envrc_content`, `github_workflow`, `justfile_content`, `makefile_content`, `process_compose`, `nx_project`) use this utility via `with_exclusions()` / `with_config_and_exclusions()` constructors.

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
