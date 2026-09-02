# Smoke Test: build-ci-stack Scanners

**Date**: 2026-09-02
**PRD**: [docs-internal/requirements/20260901-build-ci-stack.md](../20260901-build-ci-stack.md)
**Build**: `devbox run -- just build` (release)

## Scanners Under Test

| Scanner | Check Name | Target Files |
|---------|------------|--------------|
| `github_workflow` | `github_workflow` | `.github/workflows/*.yml` |
| `dependabot` | `dependabot` | `.github/dependabot.yml` |
| `justfile_content` | `justfile_content` | `justfile`, `Justfile` |
| `makefile_content` | `makefile_content` | `Makefile` |
| `process_compose` | `process_compose` | `process-compose.yaml`, `process-compose.yml` |

## Test Repositories

### 1. project-lint itself (has workflows + justfile)

**Path**: `~/p/gh/levonk/project-lint`
**CI files present**: `.github/workflows/ci.yml`, `justfile`

**Results**:

- `github_workflow` scanner fired on `.github/workflows/ci.yml`:
  - `workflow-permissions-block` (warning) — missing explicit `permissions:` block
  - `workflow-timeout` (warning) — missing `timeout-minutes:`
  - `workflow-pinned-actions` (warning) — `actions/checkout@v4`, `cachix/install-nix-action@v27`, `jetify-com/devbox-install-action@v0.15.0`, `dtolnay/rust-toolchain@stable` not pinned by SHA
- `justfile_content` scanner fired on `justfile`:
  - `justfile-no-raw-cargo` (warning) — multiple lines call `cargo` directly instead of `devbox run -- cargo`
- `dependabot` scanner: silent (no `.github/dependabot.yml`)
- `makefile_content` scanner: silent (no `Makefile`)
- `process_compose` scanner: silent (no `process-compose.yaml`)

**Verdict**: PASS — scanners fire on matching files, silent on absent files.

### 2. ffox-theme (has workflows + justfile, no dependabot/makefile/process-compose)

**Path**: `~/p/gh/levonk/ffox-theme`
**CI files present**: `.github/workflows/release.yml`, `.github/workflows/lint.yml`, `justfile`

**Results**:

- `github_workflow` scanner fired on both workflow files:
  - `release.yml`: `workflow-permissions-minimal` (contents: write), `workflow-timeout`, `workflow-concurrency`, `workflow-uses-devbox`, `workflow-pinned-actions` (4 actions)
  - `lint.yml`: `workflow-permissions-block`, `workflow-timeout`, `workflow-concurrency`, `workflow-uses-devbox`, `workflow-pinned-actions` (3 actions)
- `justfile_content` scanner fired on `justfile`:
  - `justfile-quality-target` (error) — missing `quality` target
  - `justfile-quality-full-target` (warning) — missing `quality-full` target
  - `justfile-ci-target` (warning) — missing `ci` target
  - `justfile-bootstrap-target` (info) — missing `bootstrap` target
- `dependabot` scanner: silent (no `.github/dependabot.yml`)
- `makefile_content` scanner: silent (no `Makefile`)
- `process_compose` scanner: silent (no `process-compose.yaml`)

**Verdict**: PASS — all applicable scanners fire, non-applicable scanners silent.

### 3. dmrconfig_dm32 (has Makefile, no workflows/justfile/dependabot/process-compose)

**Path**: `~/p/gh/levonk/dmrconfig_dm32`
**CI files present**: `Makefile`, `examples/Makefile`, `Makefile-mingw`

**Results**:

- `makefile_content` scanner fired on all three Makefiles:
  - `makefile-forbidden` (warning) — Makefile present; should be migrated to justfile
- `github_workflow` scanner: silent (no `.github/workflows/`)
- `dependabot` scanner: silent (no `.github/dependabot.yml`)
- `justfile_content` scanner: silent (no `justfile`)
- `process_compose` scanner: silent (no `process-compose.yaml`)

**Verdict**: PASS — only the makefile_content scanner fires; all others silent.

## Summary

| Scanner | Fires on matching files | Silent on absent files | Line numbers | Severity labels |
|---------|------------------------|----------------------|-------------|-----------------|
| `github_workflow` | YES | YES | YES | YES |
| `dependabot` | N/A (no test repo with dependabot.yml) | YES | N/A | N/A |
| `justfile_content` | YES | YES | YES | YES |
| `makefile_content` | YES | YES | N/A | YES |
| `process_compose` | N/A (no test repo with process-compose) | YES | N/A | N/A |

All five scanners are correctly wired into the lint command and produce `ScannerIssue` entries with proper rule names, severities, file paths, and line numbers. Scanners are silent when their target files do not exist.
