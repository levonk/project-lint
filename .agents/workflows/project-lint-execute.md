---
workflow: "Project-Lint Execute"
slug: "project-lint-execute"
description: "Execute a development task (new scanner, rule, fix, or refactor) in the project-lint Rust repo end-to-end: plan, implement, verify, review, commit, and ship a PR"
use: "When creating or updating a scanner, modular rule, hook handler, or fixing a bug in the project-lint repo (~/p/gh/levonk/project-lint/), to ensure the change follows the project's Rust conventions and the quality gate stays green"
date:
  created: "2026-07-24"
  knowledge-basis: "2026-08-30"
  last-used: "2026-08-30"
tags:
  - "ai/workflow/project-lint/execute"
  - "rust"
  - "scanner"
  - "linter"
  - "tree-sitter"
  - "build"
see-also:
  - skill: "git-repository-management"
    relationship: "complement"
    description: "Commit conventions used when committing the execute result"
  - skill: "github-pr"
    relationship: "dependency"
    description: "PR creation skill used in step 9 — discovers contribution standards, validates PR body, posts via gh --body-file"
  - skill: "code-quality-validation"
    relationship: "dependency"
    description: "Source for the consolidated review checklist used in step 7 — the quality bar the change must clear (fmt, clippy, tests, doc tests)"
  - skill: "code-review-guidance"
    relationship: "dependency"
    description: "Source for the consolidated review checklist used in step 7 — the per-diff code review checklist applied to the change"
  - skill: "project-adopter"
    relationship: "dependency"
    description: "Source for the consolidated review checklist used in step 7 — the project-adoption conventions the change must follow (devbox/just/direnv, AGENTS.md chain)"
  - skill: "unit-test-writing"
    relationship: "complement"
    description: "Roy Osherove-style unit test authoring guidance for the `mod tests` blocks colocated with each scanner"
---

# Project-Lint Execute

## Operation

1. **Initialize**: The root `AGENTS.md` is already in your context as an
   always-on rule — do not re-read it. Instead, identify every file or
   directory you expect to touch, walk from the repo root to each target
   path, and read every child `AGENTS.md` found along each route (per the
   Usage Protocol in the root `AGENTS.md`). The child docs are the local
   contracts that control work details.

   For project-lint the relevant contracts are:
   - `AGENTS.md` (repo root) — architecture, scanner registration flow,
     Rust conventions (2021 edition, `tracing`, `anyhow`/`thiserror`,
     colocated `mod tests`).
   - `project-lint-core/AGENTS.md` if present — core crate contracts.
   - Any `docs-internal/` or `internal-docs/` notes referenced by the
     target module.

2. **Plan**: Identify the task type the user wants to execute. Common
   project-lint task types:
   - **New scanner** — follow "Adding a New Scanner" in `AGENTS.md`
     (create `src/<name>.rs`, register in `src/lib.rs` and `src/main.rs`,
     integrate into `src/commands/lint.rs::run`, optionally add a toggle
     in `src/config.rs`).
   - **Naming dictionary update** — edit `exact_mismatches` /
     `expected_names` in `src/file_naming.rs`.
   - **New AST rule** — extend `ASTAnalyzer` in `src/ast.rs` and define
     new `tree-sitter` queries.
   - **Hook / event handling** — extend `src/hooks/` (event model,
     mappers, engine) and `src/commands/hook.rs`.
   - **Bug fix / refactor** — trace the failing path, add a regression
     test in the affected module's `mod tests`, then fix.

   If the project's CLI ticket system is in use, run `tk help` and pick
   the next ticket per the project's prioritization. Otherwise, capture
   the task as a `todo_write` list before proceeding.

3. **Apply**: Implement the change following the project's Rust
   conventions:
   - 2021 edition, idiomatic Rust, compact code (collapse duplicate
     else branches, share abstractions).
   - Logging via `tracing` — `debug!` for internal flow, `info!` for
     user-facing status.
   - Errors: `anyhow` for top-level, `thiserror` for custom error types
     in `src/utils.rs`.
   - Tests: colocated `mod tests` in the same file. Use `tempfile` and
     `assert_fs` for filesystem-related tests.
   - Do NOT add or remove comments unless asked. Preserve existing
     comments when editing.
   - If the scanner supports auto-fixing, implement `apply_fixes` so the
     `lint` command's central `--fix` / `--dry-run` logic can drive it.

4. **Verify**: Build and run the project's quality gate locally. The
   fast mode (fmt check + clippy + tests) is the pre-commit bar; full
   mode adds doc tests, bench compile-check, and `cargo audit` for CI
   parity.

   ```bash
   # Fast (pre-commit): fmt check + clippy + tests
   devbox run -- just quality

   # Full (CI parity): + doc tests + bench compile + audit
   devbox run -- just quality-full
   ```

   Equivalently, `scripts/run-quality-checks.sh` re-execs through
   devbox automatically and accepts `--full`. The CI workflow
   (`.github/workflows/ci.yml`) runs `devbox run -- just ci`, which
   maps to `quality_full_impl` — the same stages as
   `just quality-full`. Local `just quality` passing is the minimum
   bar before moving to step 5.

   If the change touches public API or behavior, also run the binary
   against an example project to sanity-check:

   ```bash
   devbox run -- just build
   ./target/release/project-lint lint examples/<relevant-example>
   ```

5. **Completeness check**: Re-read the plan from step 2 and tick off
   every item: module created/edited, registration in `src/lib.rs` and
   `src/main.rs`, integration into `src/commands/lint.rs::run`, config
   toggle in `src/config.rs` (if applicable), `AGENTS.md` architecture
   section updated if a new module was added, `mod tests` covering the
   new behavior, and any `docs-internal/` notes for non-obvious
   design decisions. If any planned item is missing or half-written,
   return to step 3 (Apply) with the gap as feedback. Do not proceed
   to step 6 with a known-incomplete change.

6. **Quality gate**: Re-run the full quality gate to confirm the
   change is green at CI parity. The build from step 4 is reused — do
   not rebuild unless the source tree changed after step 4.

   ```bash
   devbox run -- just quality-full
   ```

   All stages must pass: rustfmt `--check`, `cargo clippy
   --workspace --all-targets`, `cargo test --workspace`, doc tests,
   `cargo bench --workspace --no-run`, and (if `cargo-audit` is
   installed) `cargo audit`. Pre-existing clippy warnings are
   currently tolerated; do not introduce new ones.

7. **Code review**: Dispatch a review subagent that reads the
   consolidated review checklist sourced from the three software-dev
   skills (code-quality-validation, code-review-guidance,
   project-adopter) and reviews the change's diff
   (`git diff --stat HEAD` for unstaged work, or
   `git diff --stat HEAD~1..HEAD` after a checkpoint commit) against
   it. The subagent should specifically check:
   - Rust idioms and the project's `tracing` / `anyhow` / `thiserror`
     conventions.
   - Test coverage in the colocated `mod tests` block — Roy Osherove
     style (readable, maintainable, trustworthy).
   - Scanner registration completeness (`lib.rs`, `main.rs`,
     `commands/lint.rs`, `config.rs`).
   - No absolute paths leaked into committed code or generated hook
     installers (per the project's recurring
     `fix(install-hook)` theme).
   - `AGENTS.md` architecture section updated if a new module was
     added.

   The review subagent returns a structured verdict
   (`REVIEW_VERDICT:CLEAN|NEEDS_FIXES|BLOCKED`).
   - `CLEAN` — proceed to step 8 (Deliver).
   - `NEEDS_FIXES` — return to step 3 (Apply) with the review findings
     as feedback. Loop until `CLEAN` or the change returns `BLOCKED`.
   - `BLOCKED` — the review found issues requiring human input.
     Present the findings to the user with the question, the options,
     the recommendation, and why (per the Agent Interaction Protocol
     in the root `AGENTS.md`). Do not commit until the user resolves
     the blocker.

8. **Deliver**: Commit the changes using the conventions defined in
   the `git-repository-management` skill
   (`~/.config/devin/skills/git-repository-management/SKILL.md`).
   Stage only the files touched by this task — the repo may have
   unrelated dirty files (e.g., `tags`, `target/`) that must not be
   swept into the commit. Follow the project's existing commit
   message style (Conventional Commits with scopes, e.g.
   `feat(scanner): add submodule_integrity scanner`,
   `fix(install-hook): ...`, `docs(handoff): ...`).

9. **Ship**: Run the `github-pr` skill
   (`~/.agents/skills/github-pr/SKILL.md`) to open a well-formed PR.
   The skill handles: discovering the project's contribution standards
   and PR templates, drafting a PR body that matches project
   conventions, presenting it for human review, posting via
   `gh --body-file`, and validating the posted body renders correctly
   (no literal `\n`, backticks preserved).

   The `github-pr` skill has `disable-model-invocation: true` — the
   `skill` tool will NOT list it. Do NOT hand-roll the PR. Instead:
   1. Run `~/.agents/skills/github-pr/scripts/refresh.sh` to load the
      skill's current body (`INSTRUCTIONS.md`).
   2. Read the script output and follow the skill's workflow — it
      covers fork/branch setup, PR body drafting, `gh --body-file`
      posting, and body validation.
   3. The skill's `gh-posting-guard` protocol prevents literal `\n`
      and stripped backticks in the posted PR body.

   The CI workflow runs `devbox run -- just ci` (`quality_full_impl`)
   on the PR — confirm it passes before requesting review. Do not
   auto-merge unless the user explicitly asks.

   **When NOT to auto-merge**:
   - The user said "do not auto-merge" or "wait for my review".
   - The PR contains destructive changes requiring explicit per-action
     approval per the project's `AGENTS.md`.
   - The quality gate (step 6) was skipped or failed — the PR is not
     verified.

## Context Declaration

### File Paths
- Source repo: `~/p/gh/levonk/project-lint/` (this repository)
- Architecture contract: `AGENTS.md` (repo root)
- Core crate: `project-lint-core/`
- Scanner modules: `src/*.rs` (e.g., `src/file_naming.rs`,
  `src/package_organization.rs`, `src/ast.rs`, `src/detection.rs`,
  `src/security.rs`, `src/typescript.rs`, `src/typecheck.rs`,
  `src/eslint.rs`)
- Command orchestration: `src/commands/lint.rs`, `src/commands/watch.rs`,
  `src/commands/hook.rs`
- Hook event model: `src/hooks/` (`mod.rs`, `mappers/`, `engine.rs`)
- Quality gate script: `scripts/run-quality-checks.sh`
- CI workflow: `.github/workflows/ci.yml`
- Examples for smoke testing: `examples/`

### External Resources
- Commit templates: <https://github.com/levonk/skills-releases/blob/main/skills/software-dev/git-repository-management/references/commit-templates.md>

### Project Information
- Project: levonk/project-lint
- Repository: <https://github.com/levonk/project-lint>
- Owner: levonk
- Language: Rust (2021 edition)
- Build system: `cargo` + `just` + `devbox` (Nix)
- CI gate: `devbox run -- just ci` (fmt + clippy + tests + doc tests +
  bench compile + audit)

<!-- vim: set ft=markdown -->
