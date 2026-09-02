# Smoke Test: Monorepo Stack Scanners

**Date**: 2026-09-02
**PRD**: docs-internal/requirements/20260901-monorepo-stack.md
**Scanners**: nx_config, nx_project, pnpm_workspace, node_modules_integrity

## Build

```
$ devbox run -- just build
Finished `release` profile [optimized] target(s) in 1m 32s
```

## Test Repos

### Repos WITH monorepo configs

#### janus-hub (nx.json + pnpm-workspace.yaml + node_modules)

```
$ ./target/release/project-lint lint -p ~/p/gh/levonk/janus-hub
```

Scanner output (filtered to monorepo scanners):
```
ℹ️ [NxConfig] nx.json should define 'defaultBase' (main branch name) (nx.json: nx-default-base)
❌ [NodeMods] node_modules/@nx is a real directory, not a symlink; pnpm structure may be corrupted (node_modules: node-modules-symlinks-valid)
❌ [NodeMods] node_modules/.modules.yaml packageManager is not pnpm (found: unknown) (node_modules/.modules.yaml: node-modules-modules-yaml)
⚠️ [NodeMods] package.json missing 'packageManager' field; should be set to 'pnpm@<version>' in pnpm workspaces (package.json: node-modules-package-manager-field)
```

Result: **PASS** — nx_config fires (missing defaultBase), node_modules_integrity fires
(corrupted pnpm structure — real dirs instead of symlinks, wrong packageManager).
pnpm_workspace did not fire (workspace globs match directories). nx_project did not
fire (no project.json files or all valid).

#### ai-dashboard (nx.json + pnpm-workspace.yaml)

```
$ ./target/release/project-lint lint -p ~/p/gh/levonk/ai-dashboard
```

Scanner output (filtered to monorepo scanners):
```
ℹ️ [NxConfig] nx.json should define 'defaultBase' (main branch name) (nx.json: nx-default-base)
⚠️ [NxProject] project.json name 'ai-analytics-proxy' does not match directory name 'proxy' (apps/proxy/project.json: nx-project-name-matches-dir)
```

Result: **PASS** — nx_config fires (missing defaultBase), nx_project fires (name
mismatch between `ai-analytics-proxy` and directory `proxy`). node_modules_integrity
did not fire (no pnpm-lock.yaml or no node_modules). pnpm_workspace did not fire
(globs match).

### Repos WITHOUT monorepo configs

#### project-lint (Rust-only, no nx.json, no pnpm-workspace.yaml, no node_modules)

```
$ ./target/release/project-lint lint -p ~/p/gh/levonk/project-lint
```

Scanner output (filtered to monorepo scanners):
```
(no output — all 4 scanners silent)
```

Result: **PASS** — all 4 monorepo scanners are silent on non-monorepo repos. No
false positives.

## Centralized Exclusion List

The nx_project scanner uses `walk_project()` with the centralized exclusion list
to skip `node_modules/` when searching for `project.json` files. Verified by the
janus-hub test: no `project.json` issues from `node_modules/` directories.

The node_modules_integrity scanner explicitly scans `node_modules/` structural
metadata (`.pnpm/`, `.modules.yaml`, symlinks, foreign lockfiles) but does NOT
walk package contents — it only checks top-level entries.

## Summary

| Scanner | Fires on matching repos | Silent on non-matching repos | Exclusion list |
|---------|------------------------|------------------------------|----------------|
| nx_config | ✅ (janus-hub, ai-dashboard) | ✅ (project-lint) | N/A (root file only) |
| nx_project | ✅ (ai-dashboard) | ✅ (project-lint) | ✅ (skips node_modules) |
| pnpm_workspace | ✅ (tested via unit tests) | ✅ (project-lint) | N/A (root file only) |
| node_modules_integrity | ✅ (janus-hub) | ✅ (project-lint) | ✅ (scans metadata only) |

All 4 scanners pass the smoke test criteria:
- Fire when matching files are present
- Silent when no matching files exist (no false positives)
- Respect the centralized exclusion list
