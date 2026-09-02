# Handoff: Add Nix cache action check to project-lint github_workflow scanner

**Date**: 2026-09-02
**Session**: Post-acryl-PR-5 — nixify generated a workflow with `DeterminateSystems/magic-nix-cache-action` that explicitly set `use-flakehub: true`, which caused FlakeHub auth failures on repos without a FlakeHub org. The nixify skill was corrected, but project-lint's `github_workflow` scanner should also catch this class of bug for any repo, not just nixify-generated ones.
**Status**: Pending — scanner update not yet implemented.

## Current State

### ✅ Completed (in nixify skill)
- **Root-cause analysis**: `magic-nix-cache-action` was used with explicit `use-flakehub: true`, which attempts FlakeHub OIDC authentication. If the GitHub org is not registered on FlakeHub, CI fails with: `Unable to authenticate to FlakeHub. Individuals must register at FlakeHub.com; Organizations must create an organization at FlakeHub.com.` Omitting `use-flakehub` entirely is safe — only explicit `use-flakehub: true` is the error.
- **nixify skill fix**: `references/advanced-features.md` updated with correct guidance (not "DO NOT add", but "do not explicitly set `use-flakehub: true` unless the project has a FlakeHub org"). `validate-pre-push.sh` now flags only explicit `use-flakehub: true` without a corresponding `flakehub-cache-action`.

### ❌ Blocking Issues
1. **project-lint does not check for cache action misconfiguration.** The `github_workflow` scanner at `project-lint-core/src/scanners/github_workflow.rs` validates permissions, pins, timeout, concurrency, etc., but has no check for Nix cache action configuration. Any repo (not just nixify-generated ones) can hit this failure.

## Git State

- **project-lint HEAD**: capture at implementation time
- **Branch**: `main` (work directly on main per AGENTS.md)
- **Date captured**: 2026-09-02

## Required Reading

Before any other action, read `/Users/micro/p/gh/levonk/project-lint/AGENTS.md` — it is the root of this project's progressively-disclosed informational files. Pay special attention to the "Adding a New Scanner" section and the `github_workflow.rs` scanner architecture.

## Project Overview

### Objective

Add a check to the existing `github_workflow` scanner (or a new dedicated check within it) that validates Nix cache action configuration in `.github/workflows/*.yml` files. The check should:

1. **Detect `magic-nix-cache-action` with explicit `use-flakehub: true`** — if `DeterminateSystems/magic-nix-cache-action` is used and `use-flakehub: true` is **explicitly set** in the step's `with:` block, AND `flakehub-cache-action` is not also present in the same workflow, flag it as an error. This is the acryl PR #5 failure mode — explicitly opting into FlakeHub auth breaks CI for orgs without FlakeHub. Omitting `use-flakehub` entirely is NOT an error.

2. **Auto-fix: add `magic-nix-cache-action` with `use-flakehub: false` if neither cache action exists** — if a Nix workflow (one that runs `nix build`, `nix flake check`, etc.) has NEITHER `magic-nix-cache-action` NOR `flakehub-cache-action`, the auto-fixer should add `magic-nix-cache-action` with `use-flakehub: false` as a step. This provides the free GitHub Actions cache speedup (30-50% CI time savings) without requiring a FlakeHub account.

### Current Status

The `github_workflow` scanner exists at `project-lint-core/src/scanners/github_workflow.rs` and already parses workflow YAML via `serde_yaml`. It checks permissions, pins, timeout, concurrency, devbox usage, pull_request_target, sudo, secret injection, and runs_on validity. The new check fits naturally into the existing `scan_workflow` method.

## Key Decisions Made

- **Check name**: `workflow-nix-cache-action` (follows existing naming pattern like `workflow-pinned-actions`, `workflow-timeout`).
- **Severity**: `warning` for missing cache action (suggestion to add), `error` for explicit `use-flakehub: true` without FlakeHub org (will break CI).
- **Detection approach**: Parse workflow YAML for `uses:` lines matching `DeterminateSystems/magic-nix-cache-action` or `DeterminateSystems/flakehub-cache-action`. For magic-nix-cache-action, check the step's `with:` block for **explicit** `use-flakehub: true`. If explicitly set to true and flakehub-cache-action is not also present in the workflow, flag as error. Omitting `use-flakehub` is NOT an error.
- **Auto-fix approach**: If a workflow runs Nix commands (`nix build`, `nix flake`, `nix run`, `nix profile`) and has no cache action, insert a `magic-nix-cache-action` step with `use-flakehub: false` after the nix installer step. This requires identifying the nix-installer step (e.g. `DeterminateSystems/nix-installer-action` or `cachix/install-nix-action`) and inserting after it.
- **Config**: Add a `require_nix_cache` boolean to `scanner_config.github_workflow` (default `false` to avoid noise on non-Nix repos). When `true`, the scanner flags Nix workflows that lack a cache action.

## Technical Context

### Stack/Tools
- Rust 2021 Edition
- `serde_yaml` for workflow parsing (already used by the scanner)
- `tracing` for logging
- The scanner already has `WorkflowFile` struct with `jobs` as `serde_yaml::Mapping`

### The two cache actions

1. **`DeterminateSystems/magic-nix-cache-action`** — free, uses GitHub Actions built-in cache. Has a `use-flakehub` input — explicitly setting `use-flakehub: true` without a FlakeHub org breaks CI (FOOTGUN). Omitting `use-flakehub` is safe. Cache is scoped to single workflow in single repo.

2. **`DeterminateSystems/flakehub-cache-action`** — paid ($20/member/month, free for OSS via support@flakehub.com). Uses FlakeHub managed cache. Cache available outside CI. Authenticated via OIDC.

### The failure mode (acryl PR #5)

```
jobs:
  nix-build:
    runs-on: ubuntu-latest
    permissions:
      id-token: write  # required for OIDC
      contents: read
    steps:
      - uses: actions/checkout@v4
      - uses: DeterminateSystems/nix-installer-action@v3
      - uses: DeterminateSystems/magic-nix-cache-action@<sha>
        with:
          use-flakehub: true   # EXPLICITLY set — breaks CI without FlakeHub org!
      - run: nix build
```

If the org is not registered on FlakeHub, the magic-nix-cache-action step fails with:
```
Unable to authenticate to FlakeHub. Individuals must register at FlakeHub.com; Organizations must create an organization at FlakeHub.com.
```

Note: omitting `use-flakehub` entirely is safe — only explicit `use-flakehub: true` is the error.

### The fix (three options)

**Option A** (free, no FlakeHub account — just remove the explicit `use-flakehub: true`):
```yaml
      - uses: DeterminateSystems/magic-nix-cache-action@<sha>
        # no use-flakehub set — safe
```

**Option B** (free, explicitly opt out of FlakeHub):
```yaml
      - uses: DeterminateSystems/magic-nix-cache-action@<sha>
        with:
          use-flakehub: false
```

**Option C** (paid, with FlakeHub account):
```yaml
      - uses: DeterminateSystems/flakehub-cache-action@<sha>
```

### Auto-fix behavior

When `require_nix_cache: true` and a Nix workflow has no cache action:
1. Find the nix-installer step (search for `nix-installer-action` or `install-nix-action` in `uses:`)
2. Insert after it:
```yaml
      - uses: DeterminateSystems/magic-nix-cache-action@<latest-sha>
        with:
          use-flakehub: false
```
3. Resolve the SHA via `gh api repos/DeterminateSystems/magic-nix-cache-action/git/refs/tags/<latest-tag>` (same pattern as nixify's `resolve-action-shas.sh`)

## Implementation Plan

1. **Add detection to `github_workflow.rs`**:
   - Add a `require_nix_cache: bool` field to `GithubWorkflowScanner` (default `false`)
   - In `scan_workflow`, after existing checks, scan for Nix commands in `run:` steps
   - If Nix commands found and no cache action present → warning (`workflow-nix-cache-missing`)
   - If `magic-nix-cache-action` present with **explicit** `use-flakehub: true` and no `flakehub-cache-action` → error (`workflow-nix-cache-flakehub-auth`). Omitting `use-flakehub` is NOT an error.

2. **Add auto-fix**:
   - In the scanner's `apply_fixes` method, if `workflow-nix-cache-missing` was reported, insert the magic-nix-cache-action step with `use-flakehub: false` after the nix-installer step
   - Resolve the SHA for the latest stable tag (7-day supply-chain safety)

3. **Add config support**:
   - Add `require_nix_cache` to `scanner_config.github_workflow` in config parsing
   - Default `false` (opt-in, since most repos are not Nix repos)

4. **Tests**:
   - Test: workflow with magic-nix-cache-action and use-flakehub: false → no issues
   - Test: workflow with magic-nix-cache-action and use-flakehub: true, no flakehub-cache-action → error
   - Test: workflow with magic-nix-cache-action and use-flakehub: true, with flakehub-cache-action → no issues
   - Test: workflow with magic-nix-cache-action and use-flakehub OMITTED → no issues (not an error)
   - Test: Nix workflow with no cache action, require_nix_cache=true → warning
   - Test: Nix workflow with no cache action, require_nix_cache=false → no issues
   - Test: non-Nix workflow with no cache action → no issues
   - Test: auto-fix inserts magic-nix-cache-action with use-flakehub: false after nix-installer

## Definition of Done

- [ ] `github_workflow.rs` has `require_nix_cache` field and config support
- [ ] Detection: `workflow-nix-cache-flakehub-auth` error for **explicit** `use-flakehub: true` without `flakehub-cache-action` (omitting `use-flakehub` is NOT an error)
- [ ] Detection: `workflow-nix-cache-missing` warning for Nix workflows without cache action (when require_nix_cache=true)
- [ ] Auto-fix: inserts magic-nix-cache-action with use-flakehub: false after nix-installer step
- [ ] Tests: all 8 test cases above pass
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes
- [ ] Config documentation updated in `docs/` if applicable
- [ ] Committed on main

## Context

This handoff originates from the nixify skill correction (skills-src commit `acb1a098` + follow-up). The nixify skill's `validate-pre-push.sh` now catches this for nixify-generated PRs, but project-lint should catch it for ALL repos — not just ones where nixify was used. The `github_workflow` scanner is the right place because it already validates workflow security and quality.
