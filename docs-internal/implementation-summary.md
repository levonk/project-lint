# Implementation Summary: All Recommended Rules

## Overview

This document summarizes the complete implementation of all recommended rules for project-lint based on job-aide ADRs and configuration file best practices.

## Modules Implemented

### 1. Package Organization Module (`src/package_organization.rs`)

**Purpose**: Enforce ADR 002 package structure

**Key Functions**:
- `validate_package_path()` - Validates path structure: `packages/{category}/{platform}/{domain}/{package-name}/{language}`
- `check_platform_boundaries()` - Prevents cross-platform imports (web ↔ node)

**Validations**:
- ✅ Category validation (core, features, services, ui)
- ✅ Platform validation (web, node, shared, any)
- ✅ Language validation (typescript, python, swift, java, go, rust)
- ✅ Platform boundary enforcement

**Tests**: 6 unit tests covering valid/invalid paths and boundary violations

---

### 2. Markdown Frontmatter Module (`src/markdown_frontmatter.rs`)

**Purpose**: Enforce ADR 20251106016 standardized frontmatter

**Key Functions**:
- `validate_frontmatter()` - Validates YAML frontmatter structure
- `is_valid_adr_id()` - Validates ADR ID format (YYYYMMDDNNN)
- `is_valid_date()` - Validates date format (YYYY-MM-DD)
- `is_valid_semver()` - Validates semantic versioning

**Validations**:
- ✅ Required fields: title, synopsis, tags
- ✅ ADR-specific fields: adr-id, status, author, dates, version
- ✅ Format validation for all fields
- ✅ ADR file detection and special validation

**Tests**: 8 unit tests covering frontmatter validation, format checks, and ADR-specific rules

---

### 3. pnpm Lockfile Module (`src/pnpm_lockfile.rs`)

**Purpose**: Enforce ADR 20251106001 pnpm-only package management

**Key Functions**:
- `check_forbidden_lockfiles()` - Detects npm, bun, yarn lockfiles
- `check_scripts_for_package_managers()` - Detects npm/yarn commands in scripts

**Validations**:
- ✅ Forbidden lockfile detection (package-lock.json, bun.lock, yarn.lock)
- ✅ pnpm-lock.yaml presence check
- ✅ npm/yarn command detection in package.json scripts
- ✅ Severity levels (error for forbidden, warn for missing)

**Tests**: 5 unit tests covering lockfile detection and script validation

---

### 4. Runtime Guards Module (`src/runtime_guards.rs`)

**Purpose**: Enforce ADR 006 browser safety guards

**Key Functions**:
- `check_unguarded_browser_access()` - Detects unguarded browser API access
- `check_runtime_guards_import()` - Validates @job-aide/runtime-guards imports

**Validations**:
- ✅ Unguarded window/document/navigator access detection
- ✅ Unguarded localStorage/sessionStorage detection
- ✅ Runtime guards import validation
- ✅ Guard function usage detection (isBrowser, assertBrowser, assertServer)

**Tests**: 6 unit tests covering browser API detection and guard validation

---

### 5. Config Validation Module (`src/config_validation.rs`)

**Purpose**: Validate configuration files (tsconfig, eslint, tailwind, package.json)

**Key Functions**:
- `validate_tsconfig()` - TypeScript configuration validation
- `validate_eslint_config()` - ESLint configuration validation
- `validate_tailwind_config()` - Tailwind CSS configuration validation
- `validate_package_json()` - Package.json validation

**Validations**:

**tsconfig.json**:
- ✅ Strict mode enforcement
- ✅ Module resolution configuration
- ✅ Path aliases validation (detects ambiguous @/*)
- ✅ rootDir/outDir configuration
- ✅ Include/exclude patterns

**eslint.config.mts**:
- ✅ File extension validation (.mts required)
- ✅ @job-aide/tools-lint-eslint-config usage
- ✅ Runtime guards plugin for web projects
- ✅ Rule severity levels

**tailwind.config.ts**:
- ✅ File extension validation (.ts or .mts)
- ✅ Content configuration presence and non-empty check
- ✅ Theme structure validation
- ✅ Plugins configuration

**package.json**:
- ✅ Type field presence and value
- ✅ Exports field validation
- ✅ npm/yarn command detection
- ✅ Dependency configuration

**Tests**: 8 unit tests covering all config file types

---

### 6. Centralized Exclusion List (`project-lint-core/src/utils.rs`)

**Purpose**: Shared utility that all WalkDir-based scanners use to skip build
artifacts, dependency directories, and VCS internals. Prevents false positives
from scanning `node_modules/`, `target/`, `dist/`, etc. Prerequisite for all
content-validation scanners.

**Key Functions**:
- `DEFAULT_EXCLUDED_DIRS` — const list of 12 always-excluded directories
- `build_exclusions(extra, allow_vendor)` — assembles the full exclusion list
  from defaults + user extras + vendor toggle
- `is_excluded_rel(rel, excluded)` — drop-in replacement for inline
  `rel_str.starts_with("target/")` checks (post-hoc filtering)
- `is_excluded_entry(entry, root, excluded)` — for `WalkDir::filter_entry`
  pruning (efficient — children of excluded dirs are never visited)
- `walk_project(root, excluded, max_depth)` — pre-filtered WalkDir iterator

**Validations**:
- ✅ Excludes `node_modules/`, `target/`, `dist/`, `build/`, `.next/`,
  `.turbo/`, `.nuxt/`, `.svelte-kit/`, `.git/`, `.devbox/gen/`, `.cache/`,
  `coverage/`
- ✅ Configurable `vendor/` exclusion (off by default, on when
  `allow_vendor = false`)
- ✅ User-provided `extra_excludes` appended to defaults (deduplicated)
- ✅ Multi-segment exclusions (`.devbox/gen`) handled correctly
- ✅ Configurable via `[scanner_config.exclusion]` TOML table

**Migrated Scanners** (all now use the shared utility):
- `rust_conventions.rs` — was inline `target/` + `.git/` check
- `dockerfile_lint.rs` — had no filtering at all
- `skill_markdown.rs` — was inline `target/` + `.git/` check
- `magic_numbers.rs` — had own exempt_dirs list (kept for scanner-specific
  dirs, layered on top of centralized list)
- `vault_security.rs` — was inline `target/` + `.git/` + `node_modules/` check
- `file_naming.rs` — had no filtering at all
- `submodule_integrity.rs` — N/A (uses git2 tree walking, not WalkDir)

**Tests**: 15 unit tests covering default list contents, vendor toggle,
extra excludes, deduplication, multi-segment matching, and WalkDir pruning

---

## Module Registration

All modules registered in:
- `src/lib.rs` - Public module exports
- `src/main.rs` - Module declarations

```rust
pub mod config_validation;
pub mod markdown_frontmatter;
pub mod package_organization;
pub mod pnpm_lockfile;
pub mod runtime_guards;
```

---

## Architecture

### Generic Detection Framework Reuse

All new modules follow the pattern established by existing modules:
- Use `regex::Regex` for pattern matching
- Return `Result<T, String>` for error handling
- Provide detailed violation messages
- Include comprehensive unit tests

### Modular Design

Each module is self-contained:
- Single responsibility (one rule set per module)
- No cross-module dependencies
- Testable in isolation
- Easy to integrate into lint command

---

## Integration Points

### Ready for Integration

The following integration points are prepared:

1. **Lint Command** (`src/commands/lint.rs`)
   - Can add `perform_package_organization_analysis()`
   - Can add `perform_markdown_frontmatter_analysis()`
   - Can add `perform_pnpm_lockfile_analysis()`
   - Can add `perform_runtime_guards_analysis()`
   - Can add `perform_config_validation_analysis()`

2. **Configuration Slices**
   - `.config/project-lint/rules/slices/package-organization.toml`
   - `.config/project-lint/rules/slices/markdown-frontmatter.toml`
   - `.config/project-lint/rules/slices/pnpm-lockfile.toml`
   - `.config/project-lint/rules/slices/runtime-guards.toml`
   - `.config/project-lint/rules/slices/config-validation.toml`

3. **Profiles**
   - `.config/project-lint/rules/profiles/package-organization.toml`
   - `.config/project-lint/rules/profiles/markdown-frontmatter.toml`
   - `.config/project-lint/rules/profiles/pnpm-lockfile.toml`
   - `.config/project-lint/rules/profiles/runtime-guards.toml`
   - `.config/project-lint/rules/profiles/config-validation.toml`

---

## Testing Coverage

### Unit Tests

Total: **33 unit tests** across all modules

- Package Organization: 6 tests
- Markdown Frontmatter: 8 tests
- pnpm Lockfile: 5 tests
- Runtime Guards: 6 tests
- Config Validation: 8 tests

### Test Execution

Run all tests:
```bash
cargo test
```

Run specific module tests:
```bash
cargo test package_organization
cargo test markdown_frontmatter
cargo test pnpm_lockfile
cargo test runtime_guards
cargo test config_validation
```

---

## Build Status

### Compilation

All modules compile successfully with:
- ✅ No compiler warnings
- ✅ All dependencies resolved
- ✅ All imports correct
- ✅ All tests pass

### Dependencies

No new external dependencies required. All modules use existing dependencies:
- `regex` - Pattern matching
- `std::path` - Path handling
- `tracing` - Logging

---

## Next Steps

### Phase 1: Integration (Immediate)
1. Create configuration slice files (.toml)
2. Create profile files (.toml)
3. Integrate analysis functions into lint command
4. Test with sample projects

### Phase 2: Documentation (Short-term)
1. Create module-specific documentation
2. Add usage examples
3. Document rule severity levels
4. Create migration guides

### Phase 3: Enhancement (Medium-term)
1. Add auto-fix capabilities
2. Implement dry-run mode
3. Add performance optimizations
4. Create IDE integrations

---

## Nix Stack Scanners (2026-09-01)

**PRD**: [docs-internal/requirements/20260901-nix-stack.md](requirements/20260901-nix-stack.md)

### Shipped

Four new scanners for Nix-based dev environment files, enhancing the existing
`dev_environment` scanner (which only checks file presence) with actual content
validation.

- **`NixFlakeScanner`** (`project-lint-core/src/scanners/nix_flake.rs`):
  Validates `flake.nix` (inputs have URLs, inputs pinned to ref/rev, outputs is
  a function, has description, no spurious `flake = false`) and `flake.lock`
  (present when flake.nix exists, fresh — all inputs have lock entries, every
  node has narHash). Parses `flake.nix` with regex (Nix is not trivially
  serde-parseable) and `flake.lock` as JSON via `serde_json`. Gated by the
  `nix_flake` check name. Configurable via `[scanner_config.nix_flake]` with
  `require_stable_nixpkgs` and `check_lock_freshness`.

- **`DevboxJsonScanner`** (`project-lint-core/src/scanners/devbox_json.rs`):
  Validates `devbox.json` as JSON — name present, packages is an object (not
  array), `$schema` present, `devbox.lock` present, GitHub packages pinned to
  rev/tag, init_hook not empty, scripts delegate to `just`, no `npx`/`bunx`/`yarn`
  in scripts or init_hook. Gated by the `devbox_json` check name. Configurable
  via `[scanner_config.devbox_json]` with `require_schema`, `require_lock`,
  `require_scripts_use_just`, `forbidden_commands`.

- **`NixShellScanner`** (`project-lint-core/src/scanners/nix_shell.rs`):
  Validates `shell.nix` and `default.nix` — uses `pkgs.mkShell`, has
  `buildInputs`/`packages`, no floating `import <nixpkgs>`, and `default.nix`
  is not a shell definition. Gated by the `nix_shell` check name. Configurable
  via `[scanner_config.nix_shell]` with `require_mkshell` and
  `forbid_floating_nixpkgs`.

- **`EnvrcContentScanner`** (`project-lint-core/src/scanners/envrc_content.rs`):
  Validates `.envrc` files — no hardcoded secrets (regex-based), uses
  `use devbox`/`use flake`, no `direnv allow` command, `watch_file devbox.json`
  when using devbox, no hardcoded absolute paths. Gated by the `envrc_content`
  check name. Configurable via `[scanner_config.envrc_content]` with
  `require_devbox`, `require_watch_file`, `secret_patterns`.

### Tests

- 12 nix_flake tests (clean flake, missing lock, missing description, outputs
  not function, floating input, unstable nixpkgs, flake=false, missing narHash,
  lock not fresh, invalid lock JSON, empty flake, silent on non-nix repo)
- 12 devbox_json tests (clean, packages as array, missing lock, missing
  name/schema, floating github, pinned github, empty init_hook, script not
  using just, forbidden npx, invalid JSON, silent, config disable)
- 8 nix_shell tests (clean, no mkshell, no buildInputs, floating nixpkgs,
  default.nix as shell, silent, empty, config disable)
- 10 envrc_content tests (clean, hardcoded secret, command substitution ok,
  missing devbox, direnv allow, missing watch_file, absolute path, silent,
  empty, flake-based)

All scanners use the centralized exclusion list (`build_exclusions()` /
`walk_project()`) for WalkDir filtering.

---

## Worktree Isolation Enforcement (2026-08-31)

**PRD**: [docs-internal/requirements/20260831-worktree-isolation.md](requirements/20260831-worktree-isolation.md)
**PRs**: [#7](https://github.com/levonk/project-lint/pull/7), [#8](https://github.com/levonk/project-lint/pull/8)

### Shipped

- **`worktree-isolation-enforcer` rule** (`project-lint-core/src/hooks/engine/mod.rs`):
  - PreToolUse: blocks direct edits (Edit/Write/MultiEdit/NotebookEdit) and
    subagent dispatch (Task/run_subagent) on protected branches in the main
    worktree. Write-tool blocking scoped by `protected_paths` (default
    `["src/**"]`); subagent dispatch never path-scoped.
  - PostToolUse: re-runs protected_paths + branch check after a write lands,
    catching writes that slipped through.
  - Stop + SubagentStop: blocks stopping with a dirty tree on a protected
    branch in the main worktree. Fires on every event — no once-per-session
    suppression.
- **Configurable `protected_branches`** — TOML field on `CustomRule`,
  defaults to `["main", "master", "trunk", "develop"]`. No longer hardcoded.
- **Git pre-commit + pre-push gates** — generated scripts block commits and
  pushes to protected branches outside a linked worktree.
- **Claude Code `settings.json`** — `install-hook --agent claude` now
  creates `.claude/settings.json` registering PreToolUse + Stop hooks,
  merging non-destructively with existing settings.
- **`install-hook --agent all`** — installs both git hooks and Claude Code
  hooks in one command.
- **Env scrubbing** — Git subprocesses clear inherited `GIT_*` vars to
  prevent hook-inherited state from corrupting worktree operations.

### Tests

- 17 worktree-isolation engine tests (tool classification, edit/subagent
  block on main, docs-write allow, feature-branch allow, linked-worktree
  allow, MultiEdit via tool_input, empty-defaults-to-src, PostToolUse
  verification, Stop/SubagentStop dirty/clean, no-suppression-on-repeat,
  configurable protected_branches).
- 3 install-hook tests (settings.json creation, non-destructive merge,
  `all` agent).

### Open enhancements

- #5 `project-lint worktree start` subcommand
- #6 Auto-install hooks on `project-lint init`
- #7 Worktree status in `lint` output
- #8 Per-branch `protected_paths` scoping
- #9 Telemetry on worktree gate hits

---

## Wire Dead Scanners (2026-09-02)

**PRD**: [docs-internal/requirements/20260901-wire-dead-scanners.md](requirements/20260901-wire-dead-scanners.md)

Three scanners existed in `project-lint-core/src/scanners/` but were never
wired into `src/commands/lint.rs::run` — dead code. This section documents the
adapter wrappers and integration work that brought them online.

### Config Validation (`config_validation.rs`)

**Purpose**: Validate tsconfig.json, eslint.config.*, tailwind.config.*, and
package.json for best-practice settings.

**Wrapper**: `ConfigValidationScanner` — walks the project root, calls the
existing `ConfigValidationRuleSet` static methods, and converts
`ConfigViolation` to `ScannerIssue` with rule names:
`tsconfig-strict-mode`, `tsconfig-module-resolution`,
`tsconfig-no-ambiguous-alias`, `tsconfig-rootdir`, `tsconfig-outdir`,
`eslint-config-extension`, `eslint-config-base`, `eslint-runtime-guards-plugin`,
`tailwind-config-extension`, `tailwind-content-present`,
`tailwind-content-not-empty`, `package-json-type-field`,
`package-json-exports-field`, `package-json-no-npm-scripts`,
`package-json-no-yarn-scripts`.

**Config**: `[scanner_config.config_validation]` — `required_eslint_base`,
`require_type_module`, `check_tailwind`.

**Tests**: 13 (8 existing static-method tests + 5 new scan() wrapper tests).

### Markdown Frontmatter (`markdown_frontmatter.rs`)

**Purpose**: Validate YAML frontmatter in `.md` files — required fields
(title, synopsis, tags), frontmatter delimiters, and ADR-specific rules.

**Wrapper**: `MarkdownFrontmatterScanner` — walks the project root for `.md`
files, calls `MarkdownFrontmatterRuleSet::validate_frontmatter`, and converts
errors to `ScannerIssue` with rule names: `md-frontmatter-present`,
`md-frontmatter-closed`, `md-frontmatter-title`, `md-frontmatter-synopsis`,
`md-frontmatter-tags`, `adr-id-required`, `adr-id-format`,
`adr-status-required`, `adr-status-valid`, `adr-date-format`,
`adr-version-format`.

**Config**: `[scanner_config.markdown_frontmatter]` — `require_frontmatter`,
`adr_dirs`.

**Tests**: 13 (6 existing static-method tests + 7 new scan() wrapper tests).

### Runtime Guards (`runtime_guards.rs`)

**Purpose**: Detect unguarded browser API access in TS/JS files.

**Wrapper**: `RuntimeGuardsScanner` — walks the project root for TS/JS files,
calls `RuntimeGuardsRuleSet::check_unguarded_browser_access`, and converts
`BrowserAccessViolation` to `ScannerIssue` with rule names:
`runtime-guard-window-access`, `runtime-guard-document-access`,
`runtime-guard-navigator-access`, `runtime-guard-localstorage-access`,
`runtime-guard-sessionstorage-access`, `runtime-guard-typeof-window`,
`runtime-guard-typeof-document`.

**Config**: `[scanner_config.runtime_guards]` — `guards_package`,
`check_extensions`.

**Tests**: 12 (6 existing static-method tests + 6 new scan() wrapper tests).

## Container Stack (2026-09-02)

**PRD**: [docs-internal/requirements/20260901-container-stack.md](requirements/20260901-container-stack.md)

### compose_lint (NEW)

**File**: `project-lint-core/src/scanners/compose_lint.rs`

**Purpose**: Lint `docker-compose*.yml` / `compose*.yml` files for container
hardening best practices using `serde_yaml` for robust YAML parsing.

**Key Functions**:
- `ComposeLintScanner::new()` — sensible defaults (digests, healthcheck,
  no-new-privileges, no-privileged, no-docker-sock all on)
- `ComposeLintScanner::with_config(...)` — toggle individual rules
- `ComposeLintScanner::with_exclusions(...)` — config + centralized exclusion list
- `scan(project_path)` — walks project for compose files, parses each with
  `serde_yaml`, checks each service against the rules

**Validations** (16 rules):
- `compose-pinned-images` (error) — images must be pinned by digest
- `compose-no-latest-tag` (error) — no `:latest` tag
- `compose-no-floating-tag` (warning) — no floating major tags without digest
- `compose-no-privileged` (error) — no `privileged: true`
- `compose-security-opt` (warning) — `security_opt: ["no-new-privileges:true"]`
- `compose-no-docker-sock-mount` (error) — no `/var/run/docker.sock` (exempt by proxy label)
- `compose-no-root-user` (warning) — must specify `user:`
- `compose-no-host-network` (warning) — no `network_mode: host`
- `compose-no-host-pid` (warning) — no `pid: host`
- `compose-cap-drop` (warning) — `cap_drop: ["ALL"]`
- `compose-readonly-filesystem` (info) — `read_only: true`
- `compose-healthcheck` (warning) — must define `healthcheck`
- `compose-restart-policy` (info) — `restart: unless-stopped` or `always`
- `compose-resource-limits` (warning) — `deploy.resources.limits` (when required)
- `compose-no-bind-0.0.0.0` (warning) — no `0.0.0.0` port bindings
- `compose-watchtower-labels` (info) — watchtower/wud labels (ops_mode only)
- `compose-parse-error` (error) — malformed YAML

**Tests**: 17 unit tests (positive, negative, edge case, ops mode, resource
limits, proxy label exemption, compose.yml/override variants, malformed/empty)

### dockerfile_lint (ENHANCED)

**File**: `project-lint-core/src/scanners/dockerfile_lint.rs`

**Purpose**: Enhanced from 3 rules to 10 rules covering the full container
hardening checklist.

**New Validations** (in addition to existing 3):
- `dockerfile-no-latest-tag` (error) — `FROM` must not use `:latest` or untagged
- `dockerfile-healthcheck` (warning) — must define `HEALTHCHECK`
- `dockerfile-apk-no-cache` (warning) — `apk add` must use `--no-cache`
- `dockerfile-apt-get-no-install-recommends` (warning) — `apt-get install` must use `--no-install-recommends`
- `dockerfile-apt-get-clean` (warning) — `apt-get install` must clean up `/var/lib/apt/lists/*`
- `dockerfile-dockerignore-present` (warning) — project with Dockerfile must have `.dockerignore`
- `dockerfile-multi-stage` (info) — Dockerfiles with `RUN` install commands should use multi-stage builds
- `dockerfile-distroless-scratch-exempt` — `scratch` and `gcr.io/distroless/static:nonroot` exempt from digest pinning

**New Constructors**:
- `with_full_config(...)` — full control over all 8 toggles + exempt list + exclusions

**Tests**: 16 unit tests (existing 3 preserved + 13 new for enhanced rules)

---

## Summary

✅ **5 new modules** implementing all recommended rules
✅ **33 unit tests** with comprehensive coverage
✅ **0 external dependencies** added
✅ **100% compilation success**
✅ **Ready for integration** into lint command
✅ **Worktree isolation enforcement** (PRs #7, #8) — 17 engine tests + 3 install tests

The implementation provides:
- **Package organization validation** (ADR 002)
- **Markdown frontmatter standardization** (ADR 20251106016)
- **pnpm enforcement** (ADR 20251106001)
- **Runtime guards for browser safety** (ADR 006)
- **Configuration file validation** (tsconfig, eslint, tailwind, package.json)
- **Worktree isolation enforcement** (configurable protected_branches, pre-commit/pre-push gates, PreToolUse/PostToolUse/Stop/SubagentStop hooks, Claude settings.json install)

All modules follow project-lint's architecture patterns and are ready for production use.
