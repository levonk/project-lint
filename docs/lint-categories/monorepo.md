# Monorepo Rules

Monorepo rules validate configuration files for Nx, pnpm workspaces, and
node_modules symlink integrity in pnpm-based monorepos.

## Overview

Monorepo rules help identify:
- Missing or incomplete `nx.json` configuration (cache reuse, target defaults)
- Nx `project.json` files with mismatched names, missing targets, or missing tags
- Invalid `pnpm-workspace.yaml` content (missing packages, bad globs, node_modules in globs)
- Corrupted `node_modules/` structures from running the wrong package manager

## Configuration

Enable monorepo scanners in `.config/project-lint/` config or profile:

```toml
[rules]
enabled_checks = [
    "nx_config",
    "nx_project",
    "pnpm_workspace",
    "node_modules_integrity",
]
```

Per-scanner tuning:

```toml
[scanner_config.nx_config]
require_named_inputs = true
require_target_defaults = true

[scanner_config.nx_project]
require_name_matches_dir = true
require_tags = false

[scanner_config.pnpm_workspace]
require_catalog = false
check_glob_matches = true

[scanner_config.node_modules_integrity]
check_symlink_structure = true
check_modules_yaml = true
check_no_foreign_lockfiles = true
require_package_manager_field = true
```

## Scanners

### nx_config

Validates `nx.json` at the project root. Silent when `nx.json` does not exist.

| Rule | Severity | Description |
|------|----------|-------------|
| `nx-named-inputs` | warning | `namedInputs` should be defined for cache reuse |
| `nx-target-defaults` | warning | `targetDefaults` should be defined for standard targets |
| `nx-default-base` | info | `defaultBase` should be defined (main branch name) |
| `nx-cacheable-operations` | info | `cacheOperations` or `targetDefaults` with `cache: true` |
| `nx-json-parse` | error | `nx.json` must be valid JSON |

#### Example

❌ **Bad** (`nx.json`):
```json
{}
```

✅ **Good** (`nx.json`):
```json
{
  "namedInputs": {
    "default": ["{projectRoot}/**/*"]
  },
  "targetDefaults": {
    "build": { "cache": true }
  },
  "defaultBase": "main"
}
```

### nx_project

Walks the project for `project.json` (Nx) files, excluding `node_modules/`
via the centralized exclusion list.

| Rule | Severity | Description |
|------|----------|-------------|
| `nx-project-name-matches-dir` | warning | `name` should match directory name |
| `nx-project-has-targets` | warning | At least one target should be defined |
| `nx-project-tags-present` | info | `tags` for dependency boundary enforcement |
| `nx-project-parse` | error | `project.json` must be valid JSON |

#### Example

❌ **Bad** (`packages/my-app/project.json`):
```json
{
  "name": "wrong-name"
}
```

✅ **Good** (`packages/my-app/project.json`):
```json
{
  "name": "my-app",
  "targets": { "build": {} },
  "tags": ["type:app"]
}
```

### pnpm_workspace

Validates `pnpm-workspace.yaml` content. Silent when the file does not exist.

| Rule | Severity | Description |
|------|----------|-------------|
| `pnpm-workspace-packages` | error | `packages:` field must be present and non-empty |
| `pnpm-workspace-globs-valid` | warning | Package globs should match at least one directory |
| `pnpm-workspace-catalog` | warning | `catalog:` section when catalog mode enabled |
| `pnpm-workspace-no-node_modules-glob` | error | Globs must not include `node_modules` |
| `pnpm-workspace-parse` | error | File must be valid YAML |

#### Example

❌ **Bad** (`pnpm-workspace.yaml`):
```yaml
packages:
  - 'node_modules/*'
```

✅ **Good** (`pnpm-workspace.yaml`):
```yaml
packages:
  - 'apps/*'
  - 'packages/*'
```

### node_modules_integrity

Detects corrupted pnpm `node_modules/` structures. Only activates when
`pnpm-lock.yaml` exists. Silent when `node_modules/` doesn't exist (not
installed).

| Rule | Severity | Description |
|------|----------|-------------|
| `node-modules-pnpm-structure` | error | `node_modules/.pnpm/` must exist |
| `node-modules-symlinks-valid` | error | Top-level packages must be symlinks |
| `node-modules-modules-yaml` | error | `.modules.yaml` must have `packageManager: pnpm` |
| `node-modules-no-npm-lock` | error | No `package-lock.json` or `yarn.lock` in `node_modules/` |
| `node-modules-package-manager-field` | warning | Root `package.json` should have `packageManager: pnpm@<version>` |

#### Common Issue

When someone accidentally runs `npm install` in a pnpm workspace:
- `node_modules/.pnpm/` is deleted (npm doesn't use content-addressable store)
- Top-level packages become real directories instead of symlinks
- `node_modules/.modules.yaml` may report `packageManager: npm`
- `node_modules/package-lock.json` may appear

Fix: `rm -rf node_modules && pnpm install`

## Out of Scope

- **Turborepo** (`turbo.json`) — future scanner
- **Lerna** (`lerna.json`) — deprecated in favor of Nx
- **Rush** (`rush.json`) — not used in scanned repos
- **pnpm store pruning** — runtime concern, not project structure
- **Dependency graph validation** — handled by `pnpm install` itself
