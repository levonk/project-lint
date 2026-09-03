---
date:
  created: "2026-09-02"
  completed: ""
  last-activity: "2026-09-02"
---

# Adopt apmw-core as detection source, replacing hardcoded manifest mappings

**Date**: 2026-09-02
**Session**: Post-research comparing project detection systems. apmw is extracting its detection engine into a reusable `apmw-core` crate published to crates.io. project-lint currently hardcodes manifest-to-ecosystem mappings in multiple scanners. This handoff covers replacing those hardcoded mappings with `apmw-core` dependency.
**Status**: In progress — handoff created, awaiting apmw-core publication. **Blocked on apmw-core being published to crates.io** (see [apmw handoff](https://github.com/levonk/apmw/blob/main/.agents/handoffs/todo/202609021309-extract-apmw-core-add-features-publish.md)).

## Current State

### Completed
- **Research complete** — apmw's detection engine is the most sophisticated of the four systems compared (confidence scoring, shared-file disambiguation, ecosystem grouping, hierarchy levels).
- **apmw-core extraction planned** — handoff created in apmw repo to extract detect+ecosystem+version into a standalone crate and publish to crates.io.
- **Dependabot scanner handoff created** — `202609021249-dependabot-scanner-modernization.md` covers adding `dependabot-missing-ecosystem` check, but currently hardcodes the manifest mapping instead of using apmw-core.

### Blocking Issues
1. **apmw-core is not yet published** — this handoff cannot be executed until `apmw-core` is available on crates.io. The apmw handoff (`202609021309-extract-apmw-core-add-features-publish.md`) must complete first.
2. **Multiple scanners hardcode manifest detection** — the dependabot scanner, language-specific scanners (rust_conventions, python_config, go_config, gradle_config), and the profiles module all independently detect project type by checking for specific files.

## Git State

**Commit at handoff**: `d770072ec49ceefd9013567e16567399d79ae369` (captured via `git rev-parse HEAD`)

## Required Reading

Before any other action, read `/Users/micro/p/gh/levonk/project-lint/AGENTS.md` — it is the root of this project's progressively-disclosed informational files. Follow its Usage Protocol and re-read the chain for any path you touch.

Also read the [apmw detection comparison](https://github.com/levonk/apmw/blob/main/internal-docs/research/202609021315-project-detection-comparison.md) for context on why apmw-core was chosen over the three crates.io alternatives.

## Project Overview

### Objective

Replace project-lint's hardcoded manifest-to-ecosystem mappings with `apmw-core` dependency calls. This makes project-lint DRY with apmw and the project-detection skill — one detection source of truth instead of three independent duplications.

### Current Status

project-lint currently detects project type in multiple places:

| Location | What it detects | How |
|----------|----------------|-----|
| `project-lint-core/src/scanners/dependabot.rs` | Which Dependabot ecosystems to check for | Hardcoded check for `.github/workflows/` existence |
| `project-lint-core/src/scanners/rust_conventions.rs` | Rust project | Checks for `Cargo.toml` |
| `project-lint-core/src/scanners/python_config.rs` | Python project | Checks for `pyproject.toml` |
| `project-lint-core/src/scanners/go_config.rs` | Go project | Checks for `go.mod` |
| `project-lint-core/src/scanners/gradle_config.rs` | Gradle project | Checks for `build.gradle` |
| `project-lint-core/src/profiles.rs` | Profile activation | `check_file_content` / `check_path` — checks for specific files |
| `project-lint-core/src/hooks/engine/mod.rs` | Package manager detection for command rewriting | Checks for `package.json`, `pnpm-lock.yaml` |
| `project-lint-core/src/dependency_checker.rs` | NPM/Cargo dependency checking | Checks for `package.json`, `Cargo.toml` |

Each of these independently implements the same "does this file exist?" logic. With `apmw-core`, they can all call `apmw_core::detect::detect(path)` and get a structured `DetectionResult` with the detected manager, ecosystem, confidence score, and evidence.

## Key Decisions Made

### Dependency: `apmw-core = "0.1"` (from crates.io)

```toml
# project-lint-core/Cargo.toml
[dependencies]
apmw-core = "0.1"
```

Not a git dependency, not a path dependency — a versioned crates.io dependency. This is the standard way to depend on published Rust libraries.

### What project-lint gets from apmw-core

| apmw-core module | project-lint use case |
|-----------------|----------------------|
| `apmw_core::detect` | Replace hardcoded file-existence checks in all scanners |
| `apmw_core::ecosystem` | Map detected managers to Dependabot ecosystem names |
| `apmw_core::version` | Replace `dependency_checker.rs` lockfile parsing |
| `apmw_core::workspace` | Detect monorepo workspaces for nx_config, pnpm_workspace scanners |
| `apmw_core::version_info` | Read version constraints for dependency_version_checker scanner |

### What project-lint does NOT get from apmw-core

- Command translations (`pip` → `uv`) — that's CLI-specific, stays in apmw binary
- Install-on-use logic — that's apmw binary
- Security scanning — that's apmw binary
- Agent/MCP server — that's apmw binary

### Migration strategy: incremental, not big-bang

Don't replace all hardcoded detection in one commit. Migrate scanner by scanner:

1. Add `apmw-core` dependency to `project-lint-core/Cargo.toml`
2. Migrate `dependabot.rs` first (it's the scanner with the new `dependabot-missing-ecosystem` check that needs the full ecosystem mapping)
3. Migrate `profiles.rs` (profile activation — central to all scanner orchestration)
4. Migrate language-specific scanners (rust_conventions, python_config, go_config, gradle_config)
5. Migrate `hooks/engine/mod.rs` (command rewriting)
6. Migrate `dependency_checker.rs` (uses apmw_core::version for lockfile parsing)

Each migration is a separate story with its own tests and verification.

### Fallback: apmw-core not installed

If `apmw-core` fails to compile or has a breaking change, project-lint should still work. The migration should keep the existing hardcoded detection as a fallback (behind a feature flag or a runtime check) until apmw-core is proven stable in production.

**Recommendation**: Don't add a fallback. `apmw-core` is a library crate with no I/O dependencies — if it compiles, it works. The risk is low. If apmw-core has a breaking change, pin the version: `apmw-core = "=0.1.2"`.

## Technical Context

### Stack/Tools
- Rust 2021 Edition, MSRV 1.70 (same as apmw-core — no version conflict)
- `serde` for serialization (shared dependency)
- `tracing` for logging (shared dependency)

### Important Files
- `project-lint-core/Cargo.toml` — ADD `apmw-core = "0.1"` dependency
- `project-lint-core/src/scanners/dependabot.rs` — REPLACE hardcoded ecosystem mapping with `apmw_core::detect` + `apmw_core::ecosystem`
- `project-lint-core/src/scanners/rust_conventions.rs` — REPLACE `Cargo.toml` existence check with `apmw_core::detect`
- `project-lint-core/src/scanners/python_config.rs` — REPLACE `pyproject.toml` check with `apmw_core::detect`
- `project-lint-core/src/scanners/go_config.rs` — REPLACE `go.mod` check with `apmw_core::detect`
- `project-lint-core/src/scanners/gradle_config.rs` — REPLACE `build.gradle` check with `apmw_core::detect`
- `project-lint-core/src/profiles.rs` — REPLACE `check_file_content` / `check_path` with `apmw_core::detect`
- `project-lint-core/src/hooks/engine/mod.rs` — REPLACE `package.json` / `pnpm-lock.yaml` checks with `apmw_core::detect`
- `project-lint-core/src/dependency_checker.rs` — REPLACE lockfile parsing with `apmw_core::version`
- `AGENTS.md` — UPDATE: document apmw-core dependency and detection source

### Environment Notes
- Run tests with `cargo test`
- Run quality checks with `scripts/run-quality-checks.sh`
- Pre-commit hook runs rustfmt + clippy

## Next Steps (Priority Order)

**BLOCKED on apmw-core publication.** Do not start until `apmw-core` is available on crates.io.

1. Add `apmw-core = "0.1"` to `project-lint-core/Cargo.toml`.
2. Migrate `dependabot.rs` — replace hardcoded ecosystem mapping with `apmw_core::detect` + `apmw_core::ecosystem` mapping.
3. Migrate `profiles.rs` — replace `check_file_content` / `check_path` with `apmw_core::detect`.
4. Migrate language-specific scanners (rust_conventions, python_config, go_config, gradle_config).
5. Migrate `hooks/engine/mod.rs` — replace package manager detection for command rewriting.
6. Migrate `dependency_checker.rs` — replace lockfile parsing with `apmw_core::version`.
7. Update `AGENTS.md` to document apmw-core as the detection source.
8. Run `cargo test` and quality checks after each migration.

## Task List

**Mark legend:**
- `[ ]` — task pending
- `[~]` — task in progress
- `[x]` — task done (verified complete)
- `[!]` — task blocked (note the blocker inline)

```markdown
- [!] Add apmw-core = "0.1" to project-lint-core/Cargo.toml (blocked: apmw-core not yet published to crates.io)
- [ ] Migrate dependabot.rs to use apmw_core::detect + apmw_core::ecosystem instead of hardcoded mapping
- [ ] Migrate profiles.rs to use apmw_core::detect instead of check_file_content / check_path
- [ ] Migrate rust_conventions.rs to use apmw_core::detect instead of Cargo.toml existence check
- [ ] Migrate python_config.rs to use apmw_core::detect instead of pyproject.toml check
- [ ] Migrate go_config.rs to use apmw_core::detect instead of go.mod check
- [ ] Migrate gradle_config.rs to use apmw_core::detect instead of build.gradle check
- [ ] Migrate hooks/engine/mod.rs to use apmw_core::detect for package manager detection
- [ ] Migrate dependency_checker.rs to use apmw_core::version for lockfile parsing
- [ ] Update AGENTS.md to document apmw-core as the detection source of truth
- [ ] Run cargo test and scripts/run-quality-checks.sh to verify all migrations
```

**Maintenance protocol (receiving session):**
1. Verify in-progress marks before starting.
2. Do NOT start until apmw-core is published — check `cargo search apmw-core`.
3. Defer to execute-upsert for execution.
4. Mark done only when verified.
5. Record blockers inline.

## Definition of Done

- [ ] **[script]** `cargo test --workspace` passes with 0 failures
- [ ] **[script]** `scripts/run-quality-checks.sh` passes (rustfmt + clippy)
- [ ] **[manual]** `apmw-core` is in `project-lint-core/Cargo.toml` as a versioned dependency (not git, not path)
- [ ] **[manual]** `dependabot.rs` uses `apmw_core::detect` instead of hardcoded file checks
- [ ] **[manual]** `profiles.rs` uses `apmw_core::detect` instead of `check_file_content` / `check_path`
- [ ] **[manual]** All language-specific scanners use `apmw_core::detect`
- [ ] **[manual]** `hooks/engine/mod.rs` uses `apmw_core::detect` for command rewriting
- [ ] **[manual]** `dependency_checker.rs` uses `apmw_core::version` for lockfile parsing
- [ ] **[manual]** `AGENTS.md` documents apmw-core as the detection source

**Not Done (common false-completion signals):**
- apmw-core added to Cargo.toml but scanners still use hardcoded checks (migration incomplete)
- scanners call apmw_core::detect but ignore the ecosystem/confidence data (just checking if detection succeeded)
- dependency_checker.rs still parses lockfiles manually instead of using apmw_core::version

## Execution Plan

Every task below is executed via the `execute-upsert` skill. **All stories are blocked until apmw-core is published.**

| Story slug | Type | Base SHA | DoD | Blocked on |
|------------|------|----------|-----|------------|
| adopt-apmw-core-dependabot | standard | d770072 | dependabot.rs uses apmw_core::detect | apmw-core published |
| adopt-apmw-core-profiles | standard | (after dependabot) | profiles.rs uses apmw_core::detect | apmw-core published |
| adopt-apmw-core-scanners | standard | (after profiles) | all language scanners use apmw_core::detect | apmw-core published |
| adopt-apmw-core-hooks | standard | (after scanners) | hooks/engine uses apmw_core::detect | apmw-core published |
| adopt-apmw-core-dep-checker | standard | (after hooks) | dependency_checker uses apmw_core::version | apmw-core published |

## Open Questions

1. Should project-lint depend on `apmw-core` with default features or `default-features = false`? (Recommendation: default features — the crate is lightweight and project-lint needs detection + ecosystem + version modules.)
2. Should the migration add a feature flag (`--features apmw-core-detection`) to toggle between hardcoded and apmw-core detection? (Recommendation: no — once migrated, the hardcoded detection is dead code. Remove it, don't feature-flag it.)

## Do Not

- Do not start this work until `apmw-core` is published to crates.io — check with `cargo search apmw-core`
- Do not use a git dependency for apmw-core — use a versioned crates.io dependency
- Do not migrate all scanners in one commit — migrate scanner by scanner with tests after each
- Do not keep the hardcoded detection as a fallback — remove it once migrated, don't feature-flag it
- Do not depend on the `apmw` binary crate — depend on `apmw-core` (the library)

## Suggested Skills

- `execute-upsert` — for executing each story with worktree-per-story discipline
- `unit-test-writing` — for writing tests for the migrated scanners
- `code-review-guidance` — for reviewing the migration before merge

## Additional Context

### Related Handoffs

- **apmw handoff**: [202609021309-extract-apmw-core-add-features-publish.md](https://github.com/levonk/apmw/blob/main/.agents/handoffs/todo/202609021309-extract-apmw-core-add-features-publish.md) — MUST complete first (publishes apmw-core)
- **apmw research**: [project-detection-comparison.md](https://github.com/levonk/apmw/blob/main/internal-docs/research/202609021315-project-detection-comparison.md) — why apmw-core was chosen over alternatives
- **project-lint dependabot handoff**: `202609021249-dependabot-scanner-modernization.md` — the dependabot scanner modernization should use apmw-core for ecosystem detection
- **skills-src handoff**: `202609021636-adopt-apmw-core-in-project-detection-skill.md` — the bash skill should call `apmw detect --json` instead of maintaining its own array
- **boilerplate handoff**: `202609021636-adopt-apmw-core-in-boilerplate-templates.md` — Copier templates should use apmw-core's ecosystem mapping
