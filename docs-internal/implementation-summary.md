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

---

## Monorepo Stack Scanners (2026-09-01)

### Nx Config Scanner (`project-lint-core/src/scanners/nx_config.rs`)

**Purpose**: Validate `nx.json` for Nx monorepo configuration — cache reuse,
target defaults, and base branch.

**Key Functions**:
- `scan()` — parses `nx.json` as JSON, checks for `namedInputs`, `targetDefaults`, `defaultBase`, and cacheable operations

**Validations**:
- `nx-named-inputs` — `namedInputs` defined (warning)
- `nx-target-defaults` — `targetDefaults` defined (warning)
- `nx-default-base` — `defaultBase` defined (info)
- `nx-cacheable-operations` — `cacheOperations` or `targetDefaults` with `cache: true` (info)
- `nx-json-parse` — valid JSON (error)

**Config**: `[scanner_config.nx_config]` with `require_named_inputs`, `require_target_defaults`

**Tests**: 9 unit tests (silent when absent, missing fields, invalid JSON, config toggles, clean file, cache operations)

### Nx Project Scanner (`project-lint-core/src/scanners/nx_project.rs`)

**Purpose**: Validate Nx `project.json` files across a monorepo.

**Key Functions**:
- `scan()` — walks project for `project.json` files (excluding `node_modules/`), validates each

**Validations**:
- `nx-project-name-matches-dir` — `name` matches directory (warning)
- `nx-project-has-targets` — at least one target defined (warning)
- `nx-project-tags-present` — `tags` for dependency boundaries (info, configurable)
- `nx-project-parse` — valid JSON (error)

**Config**: `[scanner_config.nx_project]` with `require_name_matches_dir`, `require_tags`

**Tests**: 10 unit tests (name match/mismatch, missing/empty targets, tags, node_modules skip, invalid JSON, clean file)

### Pnpm Workspace Scanner (`project-lint-core/src/scanners/pnpm_workspace.rs`)

**Purpose**: Validate `pnpm-workspace.yaml` content — packages field, glob validity, catalog mode.

**Key Functions**:
- `scan()` — parses `pnpm-workspace.yaml` as YAML, validates packages globs and catalog

**Validations**:
- `pnpm-workspace-packages` — `packages:` field present and non-empty (error)
- `pnpm-workspace-globs-valid` — globs match at least one directory (warning)
- `pnpm-workspace-catalog` — `catalog:` section when catalog mode enabled (warning, configurable)
- `pnpm-workspace-no-node_modules-glob` — no `node_modules` in package globs (error)
- `pnpm-workspace-parse` — valid YAML (error)

**Config**: `[scanner_config.pnpm_workspace]` with `require_catalog`, `check_glob_matches`

**Tests**: 10 unit tests (silent when absent, missing packages, clean workspace, glob matching, node_modules glob, catalog, invalid YAML, .yml extension, empty packages)

### Node Modules Integrity Scanner (`project-lint-core/src/scanners/node_modules_integrity.rs`)

**Purpose**: Detect corrupted pnpm `node_modules/` structures (e.g. when `npm install` is run in a pnpm workspace).

**Key Functions**:
- `scan()` — checks `node_modules/` structure when `pnpm-lock.yaml` exists
- `check_pnpm_structure()` — verifies `.pnpm/` dir and symlink structure
- `check_modules_yaml_file()` — validates `.modules.yaml` has `packageManager: pnpm`
- `check_foreign_lockfiles()` — detects `package-lock.json` / `yarn.lock` in `node_modules/`
- `check_package_manager_field()` — validates root `package.json` has `packageManager: pnpm@<version>`

**Validations**:
- `node-modules-pnpm-structure` — `.pnpm/` directory exists (error)
- `node-modules-symlinks-valid` — top-level packages are symlinks (error)
- `node-modules-modules-yaml` — `.modules.yaml` exists with `packageManager: pnpm` (error)
- `node-modules-no-npm-lock` — no foreign lockfiles in `node_modules/` (error)
- `node-modules-package-manager-field` — root `package.json` has `packageManager` field (warning)

**Config**: `[scanner_config.node_modules_integrity]` with `check_symlink_structure`, `check_modules_yaml`, `check_no_foreign_lockfiles`, `require_package_manager_field`

**Tests**: 13 unit tests (silent without pnpm-lock/node_modules, missing .pnpm, real dir vs symlink, .modules.yaml missing/wrong/pnpm, foreign lockfile, package manager field missing/wrong/correct, config disable all)
