---
date:
  created: "2026-09-02"
  completed: ""
  last-activity: "2026-09-02"
---

# Modernize Dependabot scanner: warn on legacy `directory` key, detect missing composite action coverage

**Date**: 2026-09-02
**Session**: Post-research on Dependabot's current capabilities. The existing `dependabot.rs` scanner validates ecosystem coverage and schedule but doesn't check for the modern `directories` key (plural with wildcards) or composite action coverage. Three repos (skills-src, project-lint, levonk-base-boilerplate) all lack dependabot.yml entirely.
**Status**: In progress — handoff created, awaiting execute-upsert execution.

## Current State

### Completed
- **Research complete** on Dependabot's `directories` vs `directory` key and composite action coverage gaps.
- **Existing scanner analyzed** — `dependabot.rs` currently checks: ecosystem coverage, schedule interval, assignees/reviewers, group config (optional). It does NOT check: `directory` vs `directories` key, composite action coverage, or wildcard usage.

### Blocking Issues
1. **Scanner doesn't warn on legacy `directory` key** — `directory` (singular) doesn't support wildcards and is the old format. The scanner should warn and offer to auto-fix by converting to `directories`.
2. **Scanner doesn't detect missing composite action coverage** — if `.github/actions/*/action.yml` exists but the github-actions entry doesn't cover `/.github/actions/*`, the scanner should flag it.
3. **No dependabot.yml in project-lint itself** — dogfooding gap.

## Git State

**Commit at handoff**: `b0391587d662d6972a7a18d95e433272f7641650` (captured via `git rev-parse HEAD`)

## Required Reading

Before any other action, read `/Users/micro/p/gh/levonk/project-lint/AGENTS.md` — it is the root of this project's progressively-disclosed informational files. Follow its Usage Protocol and re-read the chain for any path you touch. Pay special attention to the "Adding a New Scanner" section and the existing `dependabot.rs` scanner code.

## Project Overview

### Objective

Extend the `dependabot` scanner (`project-lint-core/src/scanners/dependabot.rs`) with three new checks:

1. **`dependabot-legacy-directory-key`** (warning, auto-fixable): Warn when a `github-actions` entry uses `directory` (singular) instead of `directories` (plural). The `directories` key supports wildcards and is the current best practice (GA since June 2024). Auto-fix: convert `directory: "/"` to `directories: ["/"]`.

2. **`dependabot-missing-composite-actions`** (warning): When `.github/actions/*/action.yml` files exist in the project but no `github-actions` entry covers `/.github/actions/*` (either via `directories` wildcard or explicit per-action entries), warn that composite actions are not being checked for updates.

3. **`dependabot-no-config`** (warning): When `.github/workflows/` exists but no `.github/dependabot.yml` is present, warn that Dependabot is not configured. (Currently the scanner silently returns empty when no config file exists.)

4. **`dependabot-missing-ecosystem`** (warning): When a package manager manifest file exists in the project but no corresponding Dependabot ecosystem entry is configured, warn that the ecosystem is not being monitored. The full manifest-to-ecosystem mapping:

| Manifest file(s) | Dependabot ecosystem |
|------------------|---------------------|
| `package.json`, `pnpm-workspace.yaml` | `npm` |
| `Cargo.toml` | `cargo` |
| `go.mod` | `gomod` |
| `requirements.txt`, `pyproject.toml`, `setup.py` | `pip` |
| `pom.xml` | `maven` |
| `build.gradle`, `build.gradle.kts` | `gradle` |
| `Dockerfile`, `Dockerfile.*` | `docker` |
| `docker-compose*.yml`, `compose*.yml` | `docker-compose` |
| `Gemfile` | `bundler` |
| `Package.swift` | `swift` |
| `.gitmodules` | `gitsubmodule` |
| `composer.json` | `composer` |
| `*.csproj`, `packages.config` | `nuget` |
| `.github/workflows/*.yml`, `.github/actions/*/action.yml` | `github-actions` |

The scanner should walk the project (using the centralized exclusion list from `utils.rs` to skip `target/`, `node_modules/`, `temp_smoke_test/`, `test-area/`, etc.) and detect these manifest files. For each detected ecosystem, check if the `dependabot.yml` has a matching `package-ecosystem` entry. If not, emit `dependabot-missing-ecosystem` warning with the ecosystem name and the manifest file that triggered detection.

Also add a `.github/dependabot.yml` to project-lint itself for dogfooding.

### Current Status

The scanner at `project-lint-core/src/scanners/dependabot.rs` uses `serde_yaml` to parse the config. The `DependabotEntry` struct currently has no `directory` or `directories` field — it only tracks `package_ecosystem`, `schedule`, `groups`, `assignees`, `reviewers`. The new checks require parsing the `directory`/`directories` fields.

## Key Decisions Made

### `directory` vs `directories` — not redundant, different capabilities

- `directory` (singular): accepts one string, no wildcards. Legacy key.
- `directories` (plural): accepts a list of strings with glob/wildcard support. Current best practice (GA June 2024).
- Both can coexist in the same config file for different ecosystems, but for `github-actions` specifically, `directories` is always preferable because it can cover composite actions via `/.github/actions/*`.

### Composite action coverage detection

The scanner should check for `.github/actions/*/action.yml` (or `action.yaml`) files. If any exist, every `github-actions` entry should either:
- Use `directories` with a `/.github/actions/*` entry, OR
- Have explicit per-action `directory` entries for each composite action directory

If neither, emit `dependabot-missing-composite-actions` warning.

### Auto-fix for legacy directory key

The auto-fix for `dependabot-legacy-directory-key` converts:
```yaml
directory: "/"
```
to:
```yaml
directories:
  - "/"
```

This is a safe transformation — `directories: ["/"]` is functionally identical to `directory: "/"` but enables future wildcard additions.

### Severity choices

- `dependabot-legacy-directory-key`: **warning** (not error — the config works, it's just not optimal)
- `dependabot-missing-composite-actions`: **warning** (composite actions silently miss updates — not breaking, but a real gap)
- `dependabot-no-config`: **warning** (missing config means no automated updates at all)

## Technical Context

### Stack/Tools
- Rust 2021 Edition
- `serde_yaml` for YAML parsing (already used in the scanner)
- `walkdir` for filesystem scanning (already a dependency)
- `tracing` for logging
- Tests: unit tests in `mod tests`, `tempfile` for filesystem tests

### Existing Scanner Code

**`project-lint-core/src/scanners/dependabot.rs`**:
- `DependabotEntry` struct (line 153): needs `directory` and `directories` fields added
- `DependabotScanner::scan()` (line 42): needs the three new checks added after the existing checks
- Auto-fix: implement `apply_fixes` method for the `dependabot-legacy-directory-key` rule

### Important Files
- `project-lint-core/src/scanners/dependabot.rs` — primary file to extend
- `project-lint-core/src/scanners/mod.rs` — scanner registry (no changes needed — scanner is already registered)
- `src/commands/lint.rs` — lint command (no changes needed — scanner is already wired)
- `AGENTS.md` — UPDATE: document the new check names in the scanner list
- `.github/dependabot.yml` — CREATE: add for dogfooding (project-lint has `.github/workflows/ci.yml`)

### Reference: Modern dependabot.yml for project-lint

project-lint has these manifest files:
- `.github/workflows/ci.yml` → `github-actions`
- `Cargo.toml` + `project-lint-core/Cargo.toml` → `cargo`
- `temp_smoke_test/` has Dockerfile, docker-compose.yml, package.json, Cargo.toml — these are test fixtures and should be excluded via the scanner's exclusion list, not monitored by Dependabot

```yaml
version: 2
updates:
  - package-ecosystem: "github-actions"
    directories:
      - "/"
    schedule:
      interval: "weekly"
    groups:
      actions:
        patterns:
          - "*"
    assignees:
      - levonk
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    assignees:
      - levonk
```

Note: project-lint has no composite actions (`.github/actions/`), so no `/.github/actions/*` entry needed. The `cargo` entry uses `directory` (singular) which is fine for cargo — the legacy key warning only applies to `github-actions` entries where `directories` provides wildcard benefit.

### Environment Notes
- Run tests with `cargo test`
- Run quality checks with `scripts/run-quality-checks.sh`
- Pre-commit hook runs rustfmt + clippy

## Next Steps (Priority Order)

1. Add `directory` and `directories` fields to `DependabotEntry` struct.
2. Add `dependabot-legacy-directory-key` check (warn on `directory` for github-actions entries, auto-fix to `directories`).
3. Add `dependabot-missing-composite-actions` check (scan for `.github/actions/*/action.yml`, warn if not covered).
4. Add `dependabot-no-config` check (warn when workflows exist but no dependabot.yml).
5. Add unit tests for all three new checks.
6. Create `.github/dependabot.yml` for project-lint dogfooding.
7. Update `AGENTS.md` scanner documentation.
8. Run `cargo test` and quality checks.

## Task List

**Mark legend:**
- `[ ]` — task pending
- `[~]` — task in progress
- `[x]` — task done (verified complete)
- `[!]` — task blocked (note the blocker inline)

```markdown
- [ ] Add directory/directories fields to DependabotEntry struct in dependabot.rs
- [ ] Add dependabot-legacy-directory-key check (warning + auto-fix: convert directory to directories for github-actions entries)
- [ ] Add dependabot-missing-composite-actions check (warning when .github/actions/*/action.yml exists but not covered)
- [ ] Add dependabot-no-config check (warning when .github/workflows/ exists but no dependabot.yml)
- [ ] Add dependabot-missing-ecosystem check (warning when manifest files exist but no matching ecosystem entry — covers npm, cargo, gomod, pip, maven, gradle, docker, docker-compose, bundler, swift, gitsubmodule, composer, nuget)
- [ ] Add unit tests for all four new checks (including ecosystem detection for each manifest type)
- [ ] Create .github/dependabot.yml for project-lint (github-actions + cargo ecosystems, directories key, groups, assignees)
- [ ] Update AGENTS.md scanner list with new check names
- [ ] Run cargo test and scripts/run-quality-checks.sh to verify
```

**Maintenance protocol (receiving session):**
1. Verify in-progress marks before starting.
2. Defer to execute-upsert for execution.
3. Mark done only when verified.
4. Record blockers inline.
5. Update the list as work reveals new tasks.

## Definition of Done

- [ ] **[script]** `cargo test --workspace` passes with 0 failures
- [ ] **[script]** `scripts/run-quality-checks.sh` passes (rustfmt + clippy)
- [ ] **[script]** `cargo test dependabot` passes — all new tests pass
- [ ] **[manual]** Scanner warns `dependabot-legacy-directory-key` when github-actions entry uses `directory` (singular)
- [ ] **[manual]** Scanner auto-fixes `directory: "/"` → `directories: ["/"]` for github-actions entries
- [ ] **[manual]** Scanner warns `dependabot-missing-composite-actions` when `.github/actions/*/action.yml` exists but not covered
- [ ] **[manual]** Scanner warns `dependabot-no-config` when `.github/workflows/` exists but no dependabot.yml
- [ ] **[manual]** `.github/dependabot.yml` exists in project-lint with `directories` key
- [ ] **[manual]** `AGENTS.md` documents the new check names

**Not Done (common false-completion signals):**
- Tests pass but the legacy-directory-key check only flags `github-actions` entries (not cargo/npm — those are fine with `directory`)
- Auto-fix converts `directory` to `directories` but doesn't preserve the original value
- Composite action check doesn't handle `action.yaml` (only `action.yml`)
- No-config check fires even when dependabot.yml doesn't exist (should be silent if no workflows either)

## Execution Plan

Every task below is executed via the `execute-upsert` skill.

| Story slug | Type | Base SHA | DoD |
|------------|------|----------|-----|
| dependabot-scanner-modernization | standard | b0391587 | Scanner extended with 3 new checks, auto-fix works, tests pass, dogfooding config added |

## Open Questions

1. Should the `dependabot-legacy-directory-key` warning apply to ALL ecosystems or only `github-actions`? (Recommendation: only `github-actions` — other ecosystems like `cargo` or `npm` don't benefit from wildcards in practice, and `directory` is still valid for them.)
2. Should the auto-fix for legacy directory key also add `/.github/actions/*` if composite actions are detected? (Recommendation: no — keep the auto-fix minimal (just convert the key). Adding composite action coverage is a separate decision the user should make.)

## Do Not

- Do not make `directory` (singular) an error — it's valid YAML and works, just not optimal for github-actions
- Do not auto-add `/.github/actions/*` in the legacy-key auto-fix — that's a separate concern
- Do not flag non-github-actions ecosystems for using `directory` — they don't need wildcards
- Do not forget `action.yaml` (with `.yaml` extension) in the composite action detection

## Suggested Skills

- `execute-upsert` — for executing the story with worktree-per-story discipline
- `unit-test-writing` — for writing tests in Roy Osherove style
- `code-review-guidance` — for reviewing the scanner changes

## Additional Context

### Research Sources

- [Dependabot multi-directory + wildcard support (June 2024)](https://github.blog/changelog/2024-06-25-simplified-dependabot-yml-configuration-with-multi-directory-key-directories-and-wildcard-glob-support/)
- [Dependabot options reference — `directories` key](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference)
- [dependabot-core#6345: Local actions in .github/actions/ are not checked](https://github.com/dependabot/dependabot-core/issues/6345)
- [dependabot-core#13660: Wildcard directories causing duplicate PRs](https://github.com/dependabot/dependabot-core/issues/13660)

### Related Handoffs

- **skills-src handoff**: `202609021249-add-dependabot-config-and-update-project-adopter.md` — adds dependabot.yml to skills-src and updates project-adopter to generate modern config
- **levonk-base-boilerplate handoff**: `202609021249-add-dependabot-template-to-boilerplate.md` — adds dependabot.yml template to Copier templates
