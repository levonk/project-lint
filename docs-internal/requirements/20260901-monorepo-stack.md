# PRD: Monorepo Stack (Nx, pnpm-workspace, node_modules Symlink Integrity)

**Date**: 2026-09-01
**Status**: implemented
**Scope**: New scanners for monorepo configuration validation: `nx.json`
and `project.json` (Nx), `pnpm-workspace.yaml` content validation
(enhancing the existing `typescript_monorepo` scanner), and
`node_modules` symlink integrity checking (detects when pnpm's
content-addressable symlinks have been corrupted by running the wrong
package manager).

## Problem

The existing `typescript_monorepo` scanner checks file extensions in a
monorepo but doesn't validate monorepo configuration content. The
existing `pnpm_lockfile` scanner checks for forbidden lockfiles but
doesn't validate `pnpm-workspace.yaml` content. And there's no scanner
for `node_modules` symlink integrity — a critical issue when someone
accidentally runs `npm install` in a pnpm workspace, which replaces
pnpm's symlinks with real copies and silently breaks the monorepo.

The scan data shows:
- 5 `nx.json` files — no validation
- 29 `project.json` (Nx) files — no validation
- 16 `pnpm-workspace.yaml` files — no content validation
- Many `node_modules/` directories — no symlink integrity check

## File Types Covered

| File type | Count | Scanner |
|-----------|-------|---------|
| `nx.json` | ~5 | nx_config |
| `project.json` (Nx) | ~29 | nx_project |
| `pnpm-workspace.yaml` | ~16 | pnpm_workspace (enhance existing) |
| `node_modules/.pnpm` | many | node_modules_integrity |
| `node_modules/.modules.yaml` | many | node_modules_integrity |

## Rules

### nx_config (check name: `nx_config`) — NEW SCANNER

#### nx.json rules
- [ ] `nx-named-inputs` — `nx.json` should define `namedInputs` for cache reuse. **Severity: warning.** Auto-fixable: no.
- [ ] `nx-target-defaults` — `nx.json` should define `targetDefaults` for standard targets (build, test, lint). **Severity: warning.** Auto-fixable: no.
- [ ] `nx-default-base` — `nx.json` should define `defaultBase` (main branch name). **Severity: info.** Auto-fixable: no.
- [ ] `nx-cacheable-operations` — `nx.json` should define `cacheOperations` or `targetDefaults` with `cache: true` for build/test. **Severity: info.** Auto-fixable: no.

### nx_project (check name: `nx_project`) — NEW SCANNER

#### project.json rules
- [ ] `nx-project-name-matches-dir` — `project.json` `name` field should match the directory name. **Severity: warning.** Auto-fixable: yes (update name).
- [ ] `nx-project-has-targets` — `project.json` should define at least one target (`build`, `test`, `lint`). **Severity: warning.** Auto-fixable: no.
- [ ] `nx-project-no-implicit-deps` — `project.json` should not use implicit dependencies (`implicitDependencies` in `nx.json` project references). Use explicit `dependsOn` in targets. **Severity: info.** Auto-fixable: no.
- [ ] `nx-project-tags-present` — `project.json` should have `tags` for dependency boundary enforcement. **Severity: info.** Auto-fixable: no.

### pnpm_workspace (check name: `pnpm_workspace`) — ENHANCE EXISTING

#### pnpm-workspace.yaml rules
- [ ] `pnpm-workspace-packages` — `pnpm-workspace.yaml` should have a `packages:` field with glob patterns. **Severity: error.** Auto-fixable: no.
- [ ] `pnpm-workspace-globs-valid` — Package globs should match at least one directory. **Severity: warning.** Auto-fixable: no.
- [ ] `pnpm-workspace-catalog` — If using pnpm catalog mode, `pnpm-workspace.yaml` should define `catalog:` with version mappings. **Severity: warning.** Auto-fixable: no. **Note**: Overlaps with existing `typescript_monorepo` catalog_mode check — consolidate.
- [ ] `pnpm-workspace-no-node_modules-glob` — `packages:` globs should not accidentally include `node_modules/`. **Severity: error.** Auto-fixable: no.

### node_modules_integrity (check name: `node_modules_integrity`) — NEW SCANNER

#### Symlink integrity rules
- [ ] `node-modules-pnpm-structure` — If `pnpm-lock.yaml` exists, `node_modules/` should contain `.pnpm/` directory (pnpm's content-addressable store). If `.pnpm/` is missing, someone ran `npm install` or `yarn install` and corrupted the pnpm structure. **Severity: error.** Auto-fixable: no (fix is `rm -rf node_modules && pnpm install`).
- [ ] `node-modules-symlinks-valid` — Top-level `node_modules/<package>` should be symlinks to `.pnpm/<package>/node_modules/<package>` (pnpm's hoisted structure). If they are real directories, the structure has been corrupted. **Severity: error.** Auto-fixable: no.
- [ ] `node-modules-modules-yaml` — `node_modules/.modules.yaml` should exist and contain `packageManager: pnpm` (not npm or yarn). **Severity: error.** Auto-fixable: no.
- [ ] `node-modules-no-npm-lock` — If `pnpm-lock.yaml` exists, `node_modules/` should NOT contain `package-lock.json` or `yarn.lock` (sign of wrong package manager run). **Severity: error.** Auto-fixable: no.
- [ ] `node-modules-package-manager-field` — Root `package.json` should have `packageManager` field set to `pnpm@<version>` in pnpm workspaces. **Severity: warning.** Auto-fixable: no.

## Implementation

### NxConfigScanner (new file: `project-lint-core/src/scanners/nx_config.rs`)

Parses `nx.json` as JSON using `serde_json`. Checks for `namedInputs`,
`targetDefaults`, `defaultBase`, `cacheOperations`.

### NxProjectScanner (new file: `project-lint-core/src/scanners/nx_project.rs`)

Walks project for `project.json` files (excluding `node_modules/`).
Parses each as JSON. Checks `name` matches directory, targets present,
tags present.

### PnpmWorkspaceScanner (enhance existing `typescript_monorepo.rs` or new file)

Parses `pnpm-workspace.yaml` as YAML. Checks `packages:` field, glob
validity, catalog mode, no `node_modules` in globs.

### NodeModulesIntegrityScanner (new file: `project-lint-core/src/scanners/node_modules_integrity.rs`)

Checks `node_modules/` directory structure:
- `.pnpm/` directory exists (if pnpm-lock.yaml present)
- Top-level packages are symlinks (not real directories)
- `.modules.yaml` exists and has `packageManager: pnpm`
- No `package-lock.json` or `yarn.lock` in `node_modules/`
- Root `package.json` has `packageManager` field

Uses `std::fs::symlink_metadata()` to check if entries are symlinks.

## Configuration

```toml
[scanner_config.nx_config]
require_named_inputs = true
require_target_defaults = true

[scanner_config.nx_project]
require_tags = false
require_name_matches_dir = true

[scanner_config.pnpm_workspace]
require_catalog = false
check_glob_matches = true

[scanner_config.node_modules_integrity]
check_symlink_structure = true
check_modules_yaml = true
check_no_foreign_lockfiles = true
require_package_manager_field = true
```

## Acceptance Criteria

- [ ] All four scanners exist with `scan()` returning `Vec<ScannerIssue>`
- [ ] All four registered in `mod.rs`, wired in `lint.rs`, config in `config.rs`, documented in `AGENTS.md`
- [ ] `NodeModulesIntegrityScanner` correctly detects corrupted pnpm structure (real dirs instead of symlinks)
- [ ] `NodeModulesIntegrityScanner` is silent when `node_modules/` doesn't exist (not installed)
- [ ] `NxProjectScanner` skips `node_modules/` (centralized exclusion list)
- [ ] Tests for each rule
- [ ] Smoke test: silent on non-monorepo repos
- [ ] Smoke test: fires on `acryl`, `bookkeep-saas`, `bizfactory` (Nx + pnpm workspaces)
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **Turborepo** — `turbo.json` validation. Future scanner (only 0-1 found in scan).
- **Lerna** — `lerna.json` validation. Deprecated in favor of Nx. Future scanner if needed.
- **Rush** — `rush.json` validation. Not used in any scanned repos. Future scanner.
- **pnpm store pruning** — checking the pnpm store for orphaned packages is a runtime concern, not a project structure concern.
- **Dependency graph validation** — checking that all internal dependencies are resolvable is handled by `pnpm install` itself, not project-lint.

## Dependencies

- **Centralized exclusion list** — `NxProjectScanner` must not scan `node_modules/` for `project.json` files. `NodeModulesIntegrityScanner` explicitly scans `node_modules/` but only the structural metadata, not package contents.
- **`serde_json` crate** — for parsing `nx.json` and `project.json`.
- **`serde_yaml` crate** — for parsing `pnpm-workspace.yaml`.
