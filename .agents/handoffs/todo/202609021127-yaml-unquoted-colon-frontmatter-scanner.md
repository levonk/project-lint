---
date:
  created: "2026-09-02"
  completed: ""
  last-activity: "2026-09-02"
---

# Add YAML unquoted-colon frontmatter validation to skill_markdown scanner

**Date**: 2026-09-02
**Session**: Post-fix handoff from skills-src — 5 SKILL.md files had unquoted colons in `description:` fields that caused the skills CLI to skip them. The bug class needs to be codified as a project-lint scanner check so it never recurs.
**Status**: In progress — handoff created, awaiting execute-upsert execution.

## Current State

### Completed
- **Root-cause analysis (skills-src side)**: 5 skill frontmatter `description:` fields contained colon-space sequences (e.g., `For skills:`, `Defaults:`, `Three branches:`) that YAML interprets as mapping syntax, causing the skills CLI to skip them silently. Fixed in skills-src commit `fd6a3a3c` by double-quoting all 5 description values.
- **Skills affected**: `ai-upsert`, `cli-tool-upsert`, `container-image-build`, `find-leads`, `lead-dossier`.
- **Pre-handoff checkpoint**: Staged monorepo-stack scanner changes committed as `e6a56614` on `main` (prior session work, not related to this handoff).

### Blocking Issues
1. **No project-lint scanner detects this bug class.** The existing `skill_markdown` scanner validates that `name`, `description`, and `version` fields are present, but its line-based parser (`line.find(':')` at `skill_markdown.rs:249`) does not detect when a value itself contains unquoted colons that would break real YAML parsing. The `markdown_frontmatter` scanner has the same limitation.

## Git State

**Commit at handoff**: `e6a56614ff3634a165c947d84bc5c0bd5392c066` (captured via `git rev-parse HEAD` after the pre-handoff commit checkpoint)

This is the exact repo state at handoff time. The receiving session can reconstruct what was done by inspecting this commit and its history:
- `git show e6a56614` — the monorepo-stack checkpoint commit
- `git log e6a56614..HEAD` — work done since the handoff (during restoration)

## Required Reading

Before any other action, read `/Users/micro/p/gh/levonk/project-lint/AGENTS.md` — it is the root of this project's progressively-disclosed informational files (JIT index, binding contracts, conventions). Follow its Usage Protocol and re-read the chain for any path you touch. Pay special attention to the "Adding a New Scanner" section and the architecture overview.

## Project Overview

### Objective

Add a validation check to the existing `skill_markdown` scanner (and optionally `markdown_frontmatter`) that detects unquoted colons in YAML frontmatter values — specifically the `description:` field in `SKILL.md` files, where a colon-space sequence inside an unquoted scalar value causes YAML parsers to interpret it as a nested mapping, breaking parsing.

Additionally, wire a git pre-commit hook (or extend the existing one) that runs this check on staged `SKILL.md` and `.md` files so the bug is caught at commit time, not just at lint time.

### Current Status

The bug class is identified and the reference fix is in skills-src. The project-lint scanner needs to be extended. The existing `skill_markdown.rs` scanner at `project-lint-core/src/scanners/skill_markdown.rs` already validates frontmatter fields but does not check for unquoted-colon values. The `markdown_frontmatter.rs` scanner at `project-lint-core/src/scanners/markdown_frontmatter.rs` has the same gap.

## Key Decisions Made

- **Scanner to extend**: `skill_markdown` is the primary target — it already validates `SKILL.md` frontmatter and is the most specific match. The `markdown_frontmatter` scanner should also be extended for general `.md` files, but the `skill_markdown` check is higher priority since the bug was found in skill files.
- **Detection approach**: After extracting the `description:` value (or any scalar frontmatter value), check if it starts with a quote (`"` or `'`). If not, scan the value for `: ` (colon-space) sequences that would cause YAML to interpret the value as a mapping. This is the exact pattern that broke the 5 skills.
- **Rule name**: `skill-frontmatter-unquoted-colon` (for skill_markdown) and `md-frontmatter-unquoted-colon` (for markdown_frontmatter).
- **Severity**: `error` — this breaks YAML parsing and causes downstream tools (skills CLI) to silently skip the file.
- **Auto-fix**: Not applicable. The fix is to wrap the value in double quotes and escape internal double quotes — this changes content semantics and should be manual (the scanner should provide the suggested fix in its message, e.g., "Quote the description value: `description: \"For skills: create...\"`").

## Technical Context

### Stack/Tools
- Rust 2021 Edition
- `project-lint-core` crate — scanner modules live in `project-lint-core/src/scanners/`
- `tracing` for logging
- `anyhow` / `thiserror` for errors
- Tests: unit tests in the same file (`mod tests`), `tempfile` for filesystem tests
- Pre-commit hook: `scripts/run-quality-checks.sh` (fast checks) + `.devin/hooks.v1.json` (IDE hook config)

### The Bug Class (Reference)

YAML plain (unquoted) scalars cannot contain `: ` (colon followed by space) because YAML interprets `key: value` as a mapping. When a frontmatter field like:

```yaml
description: For skills: create from scratch or update existing AI guidance artifacts.
```

is parsed by a real YAML parser, it sees `description: For skills:` as a mapping with key `description` and value `For skills:` (which is itself a mapping key without a value), which is invalid. The skills CLI silently skips files that fail to parse.

The fix is to quote the value:

```yaml
description: "For skills: create from scratch or update existing AI guidance artifacts."
```

### Existing Scanner Code (Where to Add the Check)

**`project-lint-core/src/scanners/skill_markdown.rs`**:
- `validate_required_frontmatter_fields()` (line 210) — currently parses frontmatter line-by-line, extracting `key: value` pairs. The value extraction at line 251 (`let value = line[colon + 1..].trim();`) does not check for unquoted colons in the value.
- The check should be added after extracting the value: if the value does not start with `"` or `'`, scan it for `: ` sequences. If found, emit an error.

**`project-lint-core/src/scanners/markdown_frontmatter.rs`**:
- `validate_frontmatter()` (line 13) — same line-based parsing pattern. The value extraction at line 48 (`let value = Self::strip_quotes(line[colon_pos + 1..].trim());`) strips quotes but does not check for unquoted colons.
- The check should be added before `strip_quotes`: if the raw value does not start with a quote and contains `: `, emit an error.

### Git Hook Integration

The existing pre-commit hook runs `scripts/run-quality-checks.sh` which runs `cargo clippy` and `rustfmt --check`. To add frontmatter validation at commit time:

1. **Option A**: Add a bash check to `scripts/run-quality-checks.sh` that scans staged `.md` files for unquoted-colon frontmatter values. This is fast and runs on every commit.
2. **Option B**: Build `project-lint` in release mode and add `project-lint lint --only skill_markdown` to the pre-commit hook. This is more thorough but slower (requires a release build).
3. **Option C**: Add a standalone script (e.g., `scripts/check-frontmatter.sh`) that does the YAML check and is called from the pre-commit hook.

**Recommended**: Option A for speed (bash one-liner on staged files), with the Rust scanner as the comprehensive check for `project-lint lint` runs.

### Important Files
- `project-lint-core/src/scanners/skill_markdown.rs` — primary scanner to extend (lines 210-291: `validate_required_frontmatter_fields`)
- `project-lint-core/src/scanners/markdown_frontmatter.rs` — secondary scanner to extend (lines 13-140: `validate_frontmatter`)
- `project-lint-core/src/scanners/mod.rs` — scanner module registry
- `src/commands/lint.rs` — lint command orchestration (already wires `skill_markdown` and `markdown_frontmatter`)
- `scripts/run-quality-checks.sh` — pre-commit quality check script (add frontmatter check here)
- `.devin/hooks.v1.json` — IDE hook config (already runs `project-lint hook --source claude` on PreToolUse/PostToolUse)
- `hooks/project-lint-hook.sh` — standalone hook script
- `AGENTS.md` — project conventions (update the scanner list in "Analysis Modules" section)

### Environment Notes
- Run tests with `cargo test` (379 tests currently passing)
- Run quality checks with `scripts/run-quality-checks.sh` (runs rustfmt + clippy)
- The scanner should work on any `SKILL.md` file; skip silently if no frontmatter exists (already handled)
- Pre-commit hook runs `scripts/run-quality-checks.sh` — any new check added there runs on every commit

## Next Steps (Priority Order)

1. Extend `skill_markdown.rs` `validate_required_frontmatter_fields()` to detect unquoted colons in the `description:` value. Add unit tests.
2. Extend `markdown_frontmatter.rs` `validate_frontmatter()` to detect unquoted colons in any scalar frontmatter value. Add unit tests.
3. Add a bash frontmatter check to `scripts/run-quality-checks.sh` for pre-commit-time validation.
4. Update `AGENTS.md` scanner list to document the new check names.
5. Run `cargo test` and `scripts/run-quality-checks.sh` to verify.

## Task List

A checkbox-tracked task list. The receiving session maintains these marks as it works. Each line is one task; do not collapse multiple tasks into one line.

**Mark legend:**
- `[ ]` — task pending (not yet started)
- `[~]` — task in progress (actively being worked)
- `[x]` — task done (verified complete)
- `[!]` — task blocked (cannot proceed; note the blocker inline)

```markdown
- [ ] Extend skill_markdown.rs to detect unquoted colons in description: frontmatter values (rule: skill-frontmatter-unquoted-colon, severity: error)
- [ ] Add unit tests for skill_markdown unquoted-colon detection (positive case: unquoted colon triggers error; negative case: quoted value passes)
- [ ] Extend markdown_frontmatter.rs to detect unquoted colons in any scalar frontmatter value (rule: md-frontmatter-unquoted-colon, severity: error)
- [ ] Add unit tests for markdown_frontmatter unquoted-colon detection
- [ ] Add bash frontmatter check to scripts/run-quality-checks.sh for pre-commit-time validation on staged .md files
- [ ] Update AGENTS.md scanner documentation to list the new check names
- [ ] Run cargo test and scripts/run-quality-checks.sh to verify all changes
```

**Maintenance protocol (receiving session):**
1. **Verify in-progress marks.** Before doing anything else, re-check every task marked `[~]`. If the work is not actually underway (no evidence in the working tree, no running process, no recent edit), demote it back to `[ ]`. A stale `[~]` is worse than an unstarted `[ ]` because it hides available work from the next agent.
2. **Defer to execute-upsert for execution.** Invoke execute-upsert with the Execution Plan — it handles worktree creation, subagent dispatch, code review, and PR landing. Do not hand-roll commits, branches, PRs, or test runs outside execute-upsert — that bypasses the worktree-per-story discipline and the binding contract.
3. **Mark done only when verified.** Flip `[~]` → `[x]` only after the task's Definition of Done checks pass (see below). Never mark `[x]` on intent alone.
4. **Record blockers inline.** When a task cannot proceed, mark it `[!]` and append the blocker in parentheses on the same line, e.g. `- [!] {task blocked (waiting on upstream API access)}`. Move on to the next `[ ]` task — do not stall the whole list on one blocker.
5. **Update the list as work reveals new tasks.** Append newly discovered tasks as `[ ]` lines in priority order. Do not silently delete tasks; if a task is no longer relevant, mark it `[x]` with a note (`- [x] {task} (obsolete: reason)`).

## Definition of Done

Before declaring the handoff's work complete, verify every item below.
Items marked **[script]** are deterministically verified by a script — if the script exits non-zero, the item is NOT done. Items marked **[manual]** require the agent to check something the scripts cannot verify. Each item is a checkbox — do not skip any.

- [ ] **[script]** `cargo test --workspace` passes with 0 failures (was 379 passing before changes)
- [ ] **[script]** `scripts/run-quality-checks.sh` passes (rustfmt + clippy + new frontmatter check)
- [ ] **[script]** `cargo test skill_markdown` passes — new unquoted-colon tests pass
- [ ] **[script]** `cargo test markdown_frontmatter` passes — new unquoted-colon tests pass
- [ ] **[manual]** The `skill_markdown` scanner emits `skill-frontmatter-unquoted-colon` error when a `description:` value contains an unquoted colon-space sequence
- [ ] **[manual]** The `skill_markdown` scanner does NOT emit an error when the `description:` value is properly quoted (double or single quotes)
- [ ] **[manual]** The `markdown_frontmatter` scanner emits `md-frontmatter-unquoted-colon` error for unquoted colon-space in any scalar frontmatter value
- [ ] **[manual]** The pre-commit hook (`scripts/run-quality-checks.sh`) catches unquoted-colon frontmatter on staged `.md` files
- [ ] **[manual]** `AGENTS.md` scanner list includes the new check names

**Not Done (common false-completion signals):**
- Tests pass but the scanner does not actually flag the reference bug case (`description: For skills: create...`)
- The check only flags the `description` field but not other scalar fields in `markdown_frontmatter`
- The pre-commit hook check was added but not tested with a real unquoted-colon file
- The scanner emits a warning instead of an error (this should be `error` severity — it breaks YAML parsing)

## Execution Plan

Every task below is executed via the `execute-upsert` skill, which enforces worktree-per-story, checkpoint commits, story branches, PRs, and clean-tree-before-stop uniformly. The receiving session invokes execute-upsert with this plan; it does not hand-roll the execution discipline.

| Story slug | Type | Base SHA | DoD |
|------------|------|----------|-----|
| yaml-unquoted-colon-scanner | standard | e6a56614 | cargo test passes, skill_markdown + markdown_frontmatter detect unquoted colons, AGENTS.md updated, pre-commit hook extended |

## Open Questions

1. Should the `markdown_frontmatter` check apply to ALL scalar frontmatter values, or only specific fields like `title`, `synopsis`, `description`? (Recommendation: all scalar values — any unquoted colon in any scalar breaks YAML parsing.)
2. Should the pre-commit hook check be a bash one-liner or should it invoke `project-lint lint --only skill_markdown,markdown_frontmatter`? (Recommendation: bash one-liner for speed; the Rust scanner is the comprehensive check for full `project-lint lint` runs.)

## Do Not

- Do not change the existing `strip_quotes` logic in `markdown_frontmatter.rs` — it is correct for its purpose (stripping quotes from already-parsed values). The new check should run BEFORE `strip_quotes` to inspect the raw value.
- Do not add a YAML parser dependency (e.g., `serde_yaml`) to `skill_markdown.rs` — the existing line-based parser is intentional (keeps the scanner lightweight). The unquoted-colon check is a simple string scan on the raw value.
- Do not auto-fix the values — quoting changes content semantics and the user should review the quoted version. The scanner message should suggest the fix.
- Do not lower the severity to `warning` — this is an `error` because it breaks YAML parsing and causes downstream tools to silently skip the file.

## Suggested Skills

- `execute-upsert` — for executing the story with worktree-per-story discipline
- `code-review-guidance` — for reviewing the scanner changes before merge
- `unit-test-writing` — for writing the unit tests in Roy Osherove style

## Additional Context

### Reference Fix (skills-src)

The reference fix was applied in skills-src commit `fd6a3a3c` on 2026-09-02. The 5 affected skills had their `description:` fields double-quoted:

```yaml
# Before (broken — YAML sees "For skills:" as a mapping key):
description: For skills: create from scratch or update existing AI guidance artifacts.

# After (fixed — quoted scalar):
description: "For skills: create from scratch or update existing AI guidance artifacts."
```

The skills CLI silently skipped these 5 skills because the YAML parser failed on the unquoted colon-space sequence. The bug was invisible until a user noticed the skills were missing from the CLI's skill list.

### Existing Scanner Architecture

Both `skill_markdown.rs` and `markdown_frontmatter.rs` use simple line-based YAML parsing (not a real YAML parser). This is intentional — it keeps the scanners lightweight and fast. The unquoted-colon check fits naturally into this pattern: after extracting the raw value string, scan it for `: ` before any quote-stripping.

### Pre-Commit Hook Architecture

The pre-commit hook runs `scripts/run-quality-checks.sh` which currently runs:
1. `rustfmt --check` on all Rust files
2. `cargo clippy --workspace --all-targets`

Adding a frontmatter check here is straightforward — a bash loop over `git diff --cached --name-only -- '*.md'` files, checking each file's frontmatter for unquoted colon-space sequences.
