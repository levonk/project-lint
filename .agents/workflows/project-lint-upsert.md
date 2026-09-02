---
workflow: "Project-Lint Upsert"
slug: "lint-upsert"
description: "Add or update a project-lint scanner end-to-end with a mandatory 5-layer completion gate: requirements doc, scanner code, integration wiring, tests, and deployment readiness. Prevents the 'written but never wired' failure mode."
use: "When creating a new scanner, rewiring an existing-but-unwired scanner, or significantly updating a scanner's rules in the project-lint repo (~/p/gh/levonk/project-lint/). Use this BEFORE project-lint-execute.md — lint-upsert governs the full upsert cycle; project-lint-execute governs the implementation quality of a single task within that cycle."
date:
  created: "2026-09-01"
  knowledge-basis: "2026-08-30"
  last-used: "2026-09-01"
tags:
  - "ai/workflow/project-lint/upsert"
  - "rust"
  - "scanner"
  - "linter"
  - "quality-gate"
  - "wiring-enforcement"
see-also:
  - workflow: "project-lint-execute"
    relationship: "successor"
    description: "After lint-upsert completes the 5-layer gate, project-lint-execute handles the build/verify/review/commit/ship steps for the implementation task"
  - skill: "git-repository-management"
    relationship: "complement"
    description: "Commit conventions used when committing the upsert result"
  - skill: "unit-test-writing"
    relationship: "complement"
    description: "Roy Osherove-style unit test authoring guidance for the colocated mod tests blocks"
---

# Project-Lint Upsert

## Purpose

This workflow exists because project-lint has a recurring failure mode:
**scanners get written but never wired into the lint command.** As of
2026-09-01, three scanners (`config_validation`, `markdown_frontmatter`,
`runtime_guards`) are fully implemented in `project-lint-core/src/scanners/`
but never called from `src/commands/lint.rs::run`. They are dead code.

The 5-layer gate below makes this failure mode impossible. A scanner is
not "done" until all five layers are green. The gate is checked at the
end and re-checked after any follow-up edit. If any layer is missing,
the workflow loops back — it does not proceed to commit/ship.

## The 5 Layers

| # | Layer | Artifact | Location |
|---|-------|----------|----------|
| 1 | **Requirements** | Dated PRD with acceptance criteria | `docs-internal/requirements/YYYYMMDD-<slug>.md` |
| 2 | **Code** | Scanner module with `scan()` + `ScannerIssue` | `project-lint-core/src/scanners/<name>.rs` |
| 3 | **Integration** | Module registered + wired + config toggle | 4 touch points (see below) |
| 4 | **Tests** | Colocated `mod tests` covering acceptance criteria | Same file as scanner |
| 5 | **Deployment** | AGENTS.md updated + docs-internal updated + smoke test | See below |

### Layer 3: Integration Touch Points

A scanner is NOT wired unless ALL FOUR of these are done:

1. **`project-lint-core/src/scanners/mod.rs`** — `pub mod <name>;`
2. **`src/commands/lint.rs`** — `use` import + `if config.is_check_enabled("<check_name>")` block calling `scanner.scan()` via `perform_scanner_issues()`
3. **`project-lint-core/src/config.rs`** — `ScannerConfig` struct field + per-scanner config struct (if configurable) + `Default` impl
4. **`AGENTS.md`** (repo root) — architecture section entry listing the new scanner, its check name, and what it validates

If any of the four is missing, the scanner is **unwired** and the gate
fails. This is the exact failure mode this workflow prevents.

## Operation

### Phase 1: Requirements (Layer 1)

1. **Determine the scanner scope.** What file types does it scan? What
   rules does it enforce? What severity per rule? Is it auto-fixable?

2. **Check for existing requirements.** Search
   `docs-internal/requirements/` for any prior PRD covering this scope.
   If one exists, update it rather than creating a duplicate. If the
   scope has expanded, note the delta.

3. **Write the PRD.** Create
   `docs-internal/requirements/YYYYMMDD-<slug>.md` following the format
   of existing requirements docs (see
   `20251219-remaining-work.md` for the checklist style, or
   `20250804initial-project-lint-requirements.md` for the full PRD
   style). The PRD must include:
   - **File types covered** (extensions + special filenames)
   - **Rules** — each rule as a checklist item with:
     - Rule name (kebab-case, matches the `rule` field in `ScannerIssue`)
     - What it checks (specific pattern, field, structure)
     - Severity (`error` / `warning` / `info`)
     - Auto-fixable? (yes/no)
     - Gating check name (e.g., `compose_lint`, `nix_flake`)
   - **Acceptance criteria** — how to verify the scanner works
   - **Out of scope** — what this scanner deliberately does NOT check
   - **Configuration** — what fields go in `ScannerConfig` and the
     `[scanner_config.<name>]` TOML table

4. **Review the PRD** against the file-type scan data. Cross-reference
   with the inventory of file types found across
   `~/p/gh/levonk/` and `~/p/gh/lrepo52/` to confirm the scanner will
   have real targets. If a file type has zero instances today but is
   planned (e.g., `.tf`, `Pulumi.yaml`, `.proto`), note it as
   "forward-looking" in the PRD — the scanner should still be built,
   gated by its check name, and silent when no matching files exist.

### Phase 2: Code (Layer 2)

5. **Follow "Adding a New Scanner" in `AGENTS.md`.** Create
   `project-lint-core/src/scanners/<name>.rs` implementing:
   - A scanner struct (e.g., `ComposeLintScanner`)
   - `pub fn new() -> Self` with sensible defaults
   - `pub fn with_config(...) -> Self` if configurable
   - `pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>>`
   - `impl Default for ...` delegating to `new()`

6. **Follow Rust conventions** (2021 edition, `tracing` for logging,
   `anyhow` for errors, compact code). Use `walkdir::WalkDir` with
   `max_depth(4)` for file discovery, matching existing scanners. Skip
   the centralized exclusion list (see Layer 5 — the exclusion list is
   a cross-cutting concern handled separately).

7. **Do NOT add or remove comments** unless the PRD specifies a comment
   requirement. Preserve existing comments when editing existing
   scanners.

### Phase 3: Integration (Layer 3)

8. **Register the module** in `project-lint-core/src/scanners/mod.rs`:
   ```rust
   pub mod <name>;
   ```

9. **Add config struct** in `project-lint-core/src/config.rs`:
   - Add a field to `ScannerConfig`: `pub <name>: Option<YourScannerConfig>`
   - Define the config struct with `#[derive(Debug, Clone, Deserialize, Default)]`
   - Document the TOML table name in a doc comment

10. **Wire into `src/commands/lint.rs::run`**:
    - Add to the `use` block at the top
    - Add a gated block following the existing pattern:
      ```rust
      if config.is_check_enabled("<check_name>") {
          debug!("Performing <name> analysis");
          let scanner = match &config.scanner_config.<name> {
              Some(c) => YourScanner::with_config(/* fields from c */),
              None => YourScanner::new(),
          };
          perform_scanner_issues("<Label>", &scanner.scan(project_path)?, &mut issues);
      }
      ```

11. **Update `AGENTS.md`** (repo root) architecture section: add the
    scanner to the "Analysis Modules" list with its check name and a
    one-line description, following the existing format.

### Phase 4: Tests (Layer 4)

12. **Write colocated `mod tests`** in the scanner file, following Roy
    Osherove style (readable, maintainable, trustworthy). Use
    `tempfile::TempDir` and `assert_fs` for filesystem tests. Each
    acceptance criterion from the PRD must have at least one test:
    - A positive test (valid file → no issues)
    - A negative test (violating file → correct issue emitted)
    - An edge case test (empty file, missing field, malformed content)

13. **Run the tests**:
    ```bash
    devbox run -- just quality
    ```
    All tests must pass. If any test fails, loop back to Phase 2.

### Phase 5: Deployment Readiness (Layer 5)

14. **Update `docs-internal/implementation-summary.md`** — add the new
    scanner to the "Modules Implemented" section with its purpose, key
    functions, validations, and test count.

15. **Update `docs/lint-categories/`** — if the scanner covers a new
    category (e.g., `nix.md`, `compose.md`, `iac.md`), create a
    category doc following the format of existing docs (e.g.,
    `docs/lint-categories/security.md`).

16. **Smoke test against real repos**. Run the binary against example
    projects from `~/p/gh/levonk/` to confirm:
    - The scanner fires when matching files are present
    - The scanner is silent when no matching files exist (no false
      positives on repos that don't use this file type)
    - The scanner respects the centralized exclusion list (does not
      scan `node_modules/`, `target/`, `dist/`, `.next/`, `.turbo/`)

    ```bash
    devbox run -- just build
    ./target/release/project-lint lint ~/p/gh/levonk/<repo-with-matching-files>
    ./target/release/project-lint lint ~/p/gh/levonk/<repo-without-matching-files>
    ```

17. **Document smoke test results** in
    `docs-internal/requirements/testing/YYYYMMDD-<slug>-smoke.md`.
    Include before/after output proving the scanner is silent on
    non-matching repos.

### Phase 6: Gate Check

18. **Run the 5-layer gate check.** Verify ALL five layers are green:

    | Layer | Check command |
    |-------|---------------|
    | 1. Requirements | `ls docs-internal/requirements/YYYYMMDD-<slug>.md` exists |
    | 2. Code | `ls project-lint-core/src/scanners/<name>.rs` exists and compiles |
    | 3. Integration | `grep '<name>' project-lint-core/src/scanners/mod.rs` AND `grep '<check_name>' src/commands/lint.rs` AND `grep '<name>' project-lint-core/src/config.rs` AND `grep '<name>' AGENTS.md` — all four must return matches |
    | 4. Tests | `devbox run -- just quality` passes (fmt + clippy + tests) |
    | 5. Deployment | `docs-internal/implementation-summary.md` updated AND smoke test doc exists |

    If ANY layer fails, **loop back to the relevant phase**. Do not
    proceed to Phase 7 with a known-incomplete scanner. This is the
    core enforcement — the workflow exists to prevent shipping
    half-wired scanners.

### Phase 7: Quality Gate & Ship

19. **Run the full quality gate** (CI parity):
    ```bash
    devbox run -- just quality-full
    ```
    All stages must pass: rustfmt `--check`, `cargo clippy
    --workspace --all-targets`, `cargo test --workspace`, doc tests,
    `cargo bench --workspace --no-run`, and (if `cargo-audit` is
    installed) `cargo audit`.

20. **Hand off to `project-lint-execute.md`** for the code review,
    commit, and PR steps. The execute workflow's steps 7-9 (code
    review, deliver, ship) apply. The lint-upsert workflow's job ends
    at the gate check — the execute workflow handles the shipping
    quality bar.

## Rewiring Existing Unwired Scanners

For scanners that already exist in `project-lint-core/src/scanners/`
but are not wired into `lint.rs::run` (the "dead scanner" failure
mode), the workflow is the same but:

- **Phase 1 (Requirements)**: Write a PRD that documents what the
  scanner already does and what acceptance criteria it should meet.
  Note that the code already exists — the PRD is retroactive.
- **Phase 2 (Code)**: Skip if the scanner code is already correct. Only
  edit if the PRD reveals missing rules or bugs.
- **Phase 3 (Integration)**: This is the primary work — wiring the
  existing module into `mod.rs`, `lint.rs`, `config.rs`, and
  `AGENTS.md`.
- **Phase 4-7**: Same as for new scanners.

## Centralized Exclusion List

All `WalkDir`-based scanners must respect the centralized exclusion
list to avoid scanning `node_modules/`, `target/`, `dist/`, `.next/`,
`.turbo/`, `.git/`, and other build artifacts. The exclusion list is
maintained as a shared utility (not per-scanner) so that adding a new
excluded directory fixes all scanners at once.

When creating a new scanner, use the shared exclusion helper rather
than implementing your own path filtering. If the shared helper does
not yet exist, note it as a prerequisite task — the exclusion list
itself follows this same 5-layer upsert workflow.

## Context Declaration

### File Paths
- Source repo: `~/p/gh/levonk/project-lint/`
- Scanner modules: `project-lint-core/src/scanners/*.rs`
- Scanner registry: `project-lint-core/src/scanners/mod.rs`
- Command orchestration: `src/commands/lint.rs`
- Config structs: `project-lint-core/src/config.rs`
- Requirements docs: `docs-internal/requirements/`
- Implementation summary: `docs-internal/implementation-summary.md`
- Lint category docs: `docs/lint-categories/`
- Architecture contract: `AGENTS.md` (repo root)
- Quality gate: `scripts/run-quality-checks.sh` + `just quality` / `just quality-full`
- CI workflow: `.github/workflows/ci.yml`

### File-Type Inventory Sources
- `~/p/gh/levonk/` — 64 repos (Rust, TS, Go, Python, Nix, Docker, Ansible)
- `~/p/gh/lrepo52/` — 19 repos (TS, JS, Python, Docker)
- `~/p/gh/levonk/skills-src/build/*/skills/` — skill definitions with
  file-type-specific rules
- `~/p/gh/{levonk,lrepo52}/.agents/workflows/*.md` — workflow
  definitions with process and file-type rules

<!-- vim: set ft=markdown -->
