# Handoff: Codify submodule-as-tree detection as a project-lint scanner

**Date**: 2026-08-12
**Session**: Post-incident — infrahub levonk submodule was accidentally converted from `160000 commit` to `040000 tree` in commit `9704d5e` (Aug 9 2026). The bug went undetected for 17 commits. A pre-commit hook was added to infrahub, but the detection logic belongs in project-lint as a reusable scanner so every repo benefits, not just infrahub.
**Status**: Completed — scanner implemented in commits `b8adce0` and `02215ad`, all DoD tasks verified `[x]`, archived to `.agents/handoffs/2026/08/`.

## Current State

### ✅ Completed
- **Root-cause analysis**: Commit `9704d5e` in infrahub accidentally ran `git add` on the `levonk/` submodule directory, converting it from a gitlink (`160000 commit`) to a regular tree (`040000 tree`). This caused 28+ submodule files to be tracked directly in the parent repo.
- **Forward-fix in infrahub**: Submodule tracking restored via `git rm --cached -r levonk/` + `git update-index --add --cacheinfo 160000,<sha>,levonk`. Verified with `git ls-tree HEAD levonk` → `160000 commit`.
- **Pre-commit hook in infrahub**: A bash pre-commit hook at `scripts/hooks/pre-commit` was created and wired via `core.hooksPath=scripts/hooks`. It writes the staged index to a tree (`git write-tree`), then checks each `.gitmodules` submodule path's mode — must be `160000 commit`, not `040000 tree`. Works for ANY submodule, not just levonk.

### ❌ Blocking Issues
1. **No project-lint scanner exists for this class of bug.** The infrahub pre-commit hook is repo-local; every other repo with submodules is unprotected. The detection logic should be codified as a `project-lint-core` scanner so `project-lint lint` catches it in any repo.

## Git State

- **Parent repo HEAD**: `02215ad6732e477e1310cb4f1a516252e4481ca5`
- **Branch**: `main`
- **Date captured**: 2026-08-12 (post-implementation)

## Required Reading

Before any other action, read `/Users/micro/p/gh/levonk/project-lint/AGENTS.md` — it is the root of this project's progressively-disclosed informational files. Pay special attention to the "Adding a New Scanner" section and the architecture overview.

## Project Overview

### Objective

Add a new scanner to `project-lint-core` that detects when a git submodule (declared in `.gitmodules`) is tracked in the parent repo's index/tree as a regular directory (`040000 tree`) instead of a gitlink (`160000 commit`). This is the same detection logic implemented in the infrahub pre-commit hook, but codified as a reusable Rust scanner that runs via `project-lint lint` on any repo.

### Current Status

The infrahub pre-commit hook (bash) is the reference implementation. It needs to be ported to Rust as a `project-lint-core` scanner module and integrated into the lint command flow.

## Key Decisions Made

- **Scanner name**: `submodule_integrity` (kebab-case for the module file: `submodule_integrity.rs`). Follows the existing scanner naming pattern (`dockerfile_lint`, `vault_security`, `ci_cd_parity`, etc.).
- **Detection approach**: Use `git2` crate (already a dependency in `project-lint-core/Cargo.toml`) to:
  1. Parse `.gitmodules` for submodule paths
  2. Walk the index (or HEAD tree) and check each submodule path's mode
  3. Flag any submodule path that is `040000` (tree) or has individual files tracked under it, instead of `160000` (commit/gitlink)
- **Severity**: `error` — this is a structural integrity violation that causes real data tracking problems.
- **Auto-fix**: Not applicable for this scanner. The fix requires `git rm --cached -r <path>` + `git update-index --add --cacheinfo 160000,<sha>,<path>`, which is destructive to the index and should be manual (with the scanner providing the exact fix command in its message).

## Technical Context

### Stack/Tools
- Rust 2021 Edition
- `git2` crate v0.20 (already in `Cargo.toml`) — provides `Repository`, `Index`, `Tree`, index entry mode constants
- `serde` + `toml` for config
- `tracing` for logging
- `anyhow` / `thiserror` for errors

### Reference Implementation (bash, from infrahub)

The infrahub pre-commit hook at `scripts/hooks/pre-commit` is the reference. Its detection logic:

1. Read `.gitmodules` via `git config -f .gitmodules --get-regexp 'path$'` → extract submodule paths
2. Write staged index to tree: `git write-tree` → `tree_sha`
3. For each submodule path, `git ls-tree "$tree_sha" -- "$sm_path"`:
   - If mode is `160000` and type is `commit` → OK
   - If mode is `040000` and type is `tree` → VIOLATION (submodule converted to directory)
   - If path is missing but files exist under `"$sm_path/"` → VIOLATION (gitlink missing, files leaked)
   - Any other mode → VIOLATION

### Important Files
- `project-lint-core/src/scanners/mod.rs` — scanner module registry (add `pub mod submodule_integrity;` and re-export)
- `project-lint-core/src/scanners/git.rs` — existing git scanner (branch checking, `GitInfo` struct) — reference for `git2` usage patterns
- `project-lint-core/src/lib.rs` — public API re-exports (add the new scanner type)
- `project-lint-core/src/config.rs` — config types (add `SubmoduleIntegrityConfig` if config-driven)
- `project-lint-core/Cargo.toml` — dependencies (`git2` already present)
- `src/commands/lint.rs` — lint command orchestration (integrate the new scanner into the run flow)
- `src/config.rs` — main config (add scanner toggle if following the pattern)

### Environment Notes
- Run tests with `cargo test`
- The scanner should work on any git repo with a `.gitmodules` file; skip silently if no `.gitmodules` exists
- Use `git2::Repository::open(path)` then `repo.index()` to get the staged index, or `repo.head().peel_to_tree()` for the HEAD tree
- `git2::IndexEntry` has a `mode` field — `0o160000` is gitlink, `0o040000` is tree

### Scanner Issue Type

Follow the existing `ScannerIssue` pattern from `scanners/mod.rs`:

```rust
pub struct ScannerIssue {
    pub scanner: String,
    pub severity: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub fix: Option<String>,
}
```

For this scanner, `file` should be the submodule path (e.g., `levonk`), and `fix` should be the exact command:
```
git rm --cached -r '<path>' && git update-index --add --cacheinfo 160000,<submodule-sha>,'<path>'
```

## Next Steps (Priority Order)

1. Create `project-lint-core/src/scanners/submodule_integrity.rs` with a `SubmoduleIntegrityScanner` struct implementing the `scan` method
2. Register the module in `project-lint-core/src/scanners/mod.rs` and re-export from `project-lint-core/src/lib.rs`
3. Add config support in `project-lint-core/src/config.rs` (toggle + severity, following existing scanner config patterns)
4. Integrate into `src/commands/lint.rs` within the `run` function
5. Add unit tests in `submodule_integrity.rs` (use `tempfile` / `assert_fs` to create a test repo with a submodule, test both correct and broken states)
6. Run `cargo test` and `cargo build` to verify
7. Optionally: add a modular rule file in `.config/project-lint/rules/` that enables this scanner with appropriate severity

## Task List

**Mark legend:**
- `[ ]` — task pending (not yet started)
- `[~]` — task in progress (actively being worked)
- `[x]` — task done (verified complete)
- `[!]` — task blocked (cannot proceed; note the blocker inline)

```markdown
- [x] Read AGENTS.md "Adding a New Scanner" section and existing scanner patterns (git.rs, dockerfile_lint.rs)
- [x] Create `project-lint-core/src/scanners/submodule_integrity.rs` with `SubmoduleIntegrityScanner`
- [x] Implement `.gitmodules` parsing via `git2` or direct TOML parse
- [x] Implement index/tree mode check: each submodule path must be mode `0o160000` (gitlink)
- [x] Implement nested-file detection: flag individual files tracked under a submodule path
- [x] Return `ScannerIssue` with severity `error`, the submodule path, and the fix command
- [x] Register module in `scanners/mod.rs` and re-export from `lib.rs`
- [x] Add config support in `config.rs` (scanner toggle + severity)
- [x] Integrate scanner into `src/commands/lint.rs` run flow
- [x] Write unit tests: correct submodule (gitlink), broken submodule (tree), missing gitlink with leaked files, no .gitmodules (skip)
- [x] Run `cargo test` — all tests pass (211 tests, 5 new)
- [x] Run `cargo build` — no warnings from new code
- [x] Commit with descriptive message (commits b8adce0 + 02215ad)
```

**Maintenance protocol (receiving session):**
1. **Verify in-progress marks.** Re-check every `[~]` task. If work is not actually underway, demote to `[ ]`.
2. **Start the next available task.** Pick the first `[ ]` task in priority order. Mark `[~]` before starting.
3. **Prefer subagents for parallel work.** When independent `[ ]` tasks exist, launch parallel `run_subagent` calls.
4. **Mark done only when verified.** Flip `[~]` → `[x]` only after verification (build passes, test passes).
5. **Record blockers inline.** Mark blocked tasks `[!]` with the blocker in parentheses.
6. **Update the list as work reveals new tasks.** Append new tasks as `[ ]` in priority order.

## Definition of Done

- [x] **[manual]** Every Task List item is `[x]` or marked `[x]` with an obsolete note
- [x] **[script]** `git status --porcelain` shows no uncommitted changes (all work committed)
- [x] **[manual]** The handoff document's Git State commit SHA matches `git rev-parse HEAD`
- [x] **[manual]** Each completed task's deliverable matches what was described
- [x] **[script]** `cargo test` passes (211 tests, 5 new for submodule_integrity)
- [x] **[script]** `cargo build` passes with no warnings from new code
- [x] **[manual]** The scanner detects the infrahub-style bug (submodule as `040000 tree`) when run against a test repo with that state — covered by `detects_tree_instead_of_gitlink` test
- [x] **[manual]** The scanner passes cleanly when run against a repo with correct submodule tracking (`160000 commit`) — covered by `clean_gitlink_has_no_errors` test

## Open Questions/Blockers
- Should the scanner check the staged index (what would be committed) or the HEAD tree (what's already committed)? The infrahub hook checks the staged index via `git write-tree`. For `project-lint lint`, checking HEAD is more useful (catches already-committed bugs), but checking the index catches pre-commit issues. **Recommendation**: check HEAD tree by default, with a config option to also check the staged index. — Impact: determines whether the scanner is a "lint existing state" or "pre-commit gate" tool.
- Should this scanner also verify that the submodule's gitlink SHA matches the actual submodule's HEAD? That's a related but different check (submodule pointer drift). — Impact: scope expansion; recommend deferring to a separate scanner.

## Do Not
- Do NOT use `std::process::Command` to shell out to `git` — use the `git2` crate (already a dependency)
- Do NOT auto-fix the index — the fix is destructive (`git rm --cached -r`) and should be manual with the scanner providing the exact command
- Do NOT hardcode submodule paths — read from `.gitmodules` so the scanner works for any repo
- Do NOT add AI attribution to commits (per project conventions)

## Suggested Skills
- `code-quality-validation` — run `cargo test` and `cargo build` as the quality gate
- `git-repository-management` — for committing the scanner changes
- `unit-test-writing` — for the test structure (Roy Osherove style: readable, maintainable, trustworthy)

## Additional Context

### The infrahub pre-commit hook (reference implementation)

The working bash implementation lives at `~/p/gh/levonk/infrahub/scripts/hooks/pre-commit`. It was tested against 4 scenarios:
1. Correct state (gitlink `160000`) → passes
2. Broken state (tree `040000`) → blocked with file listing and fix command
3. Normal empty commit → passes
4. `git commit` with broken index → blocked by hook through git's hook mechanism

The Rust scanner should produce equivalent detection with structured `ScannerIssue` output instead of stderr text.

### Related work in other repos

Two companion handoffs are being created in parallel:
- **skills-src**: Run `skill-src-upsert` on `project-adopter` to add pre-commit hook installation functionality (so `adopt-project.sh` sets up `core.hooksPath` and installs a submodule-integrity pre-commit hook as part of project adoption)
- **levonk-base-boilerplate**: Update the `repo/git-repo` and `repo/pnpm-monorepo` boilerplate templates to include the pre-commit hook and `core.hooksPath` setup in their generated projects

These three handoffs are independent and can be worked in parallel. The project-lint scanner is the canonical detection logic; the skills-src and boilerplate handoffs distribute the pre-commit hook as a first-line defense while project-lint provides the reusable lint-time check.
