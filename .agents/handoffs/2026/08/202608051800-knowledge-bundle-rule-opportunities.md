# project-lint: Knowledge-Bundle-Driven Rule & Test Hardening

**Date**: 2026-08-05
**Session**: Examined ~/p/gh/levonk/skills-src/build/*/knowledge bundles for rule opportunities, audited project-lint tests against bundle testing rules, and identified missing bundle content. Three subagents ran in parallel: rule mining, test compliance audit, and bundle gap analysis.
**Status**: In progress — analysis complete, implementation pending

## Current State

### Completed
- Read project-lint AGENTS.md and confirmed architecture (scanners in `project-lint-core/src/scanners/`, hooks in `project-lint-core/src/hooks/`, commands in `src/commands/`).
- Mined 25 high-value custom-rule TOML drafts, 6 new-scanner proposals, and 9 existing-scanner extension proposals from 13 knowledge bundles (12 flagged out-of-scope).
- Audited 23 test files (98 tests) against `rust-development-practices/testing-strategy.md`, `quality-gates.md`, `error-handling.md`, `cicd-testing-practices/*`, and `dev-environment-practices/*`.
- Identified 17 high-priority + 10 medium-priority + 3 low-priority missing concepts in the knowledge bundles themselves (cross-referenced to project-lint source files that need the guidance).

### Blocking Issues
1. The drafted TOML rules use fields (`enabled_if_path_exists`, `disabled_if_path_exists`, `required`, `exception_pattern`, `exclude_patterns`, `condition`) that are NOT confirmed to exist in `project-lint-core/src/config.rs` `CustomRule` struct. Schema support must be verified before any rule file is shipped.
2. The test audit recommends adding 6 dev-dependencies (`tokio-test`, `serial_test`, `criterion`, `proptest`, `assert_cmd`, `predicates`) — adding deps requires confirming versions are ≥7 days old per AGENTS.md supply-chain guidance.
3. No pre-commit hook or CI workflow exists in project-lint today, so "pre-commit/CI parity" work has no anchor to extend.

## Git State

**Commit at handoff**: `278df1252bd3466caf900642064f44a0ff806327` (branch `main`)

This is the exact repo state at handoff time. Reconstruct via:
- `git show 278df12` — what was at handoff
- `git log 278df12..HEAD` — work done since

## Required Reading

Before any other action, read `{REPO_ROOT}/AGENTS.md` — it is the root of this project's progressively-disclosed informational files (binding contracts, scanner registration steps, naming dictionary location, testing conventions). Follow its Usage Protocol and re-read the chain for any path you touch (e.g. `project-lint-core/src/scanners/AGENTS.md` if one exists, else the nearest parent).

Also read the source knowledge bundles that drove these findings (read-only rendered copies):
- `~/p/gh/levonk/skills-src/build/current/knowledge/rust-development-practices/` (overview.md, testing-strategy.md, quality-gates.md, error-handling.md, async-patterns.md, project-structure.md)
- `~/p/gh/levonk/skills-src/build/current/knowledge/cicd-testing-practices/` (overview.md, pre-commit-ci-parity.md, shared-quality-scripts.md)
- `~/p/gh/levonk/skills-src/build/current/knowledge/dev-environment-practices/` (overview.md, mandatory-testing-workflow.md, standard-developer-ux-flow.md, branch-tag-hygiene.md, shell-scripting-best-practices.md)
- `~/p/gh/levonk/skills-src/build/current/knowledge/secrets-egress-security/`
- `~/p/gh/levonk/skills-src/build/current/knowledge/devsecops-codeguard/`

## Project Overview

### Objective
Convert knowledge-bundle best practices into enforceable project-lint rules, fix test-compliance gaps the bundles require, and feed bundle-gap findings back to skills-src (tracked in the companion handoff in skills-src).

### Current Status
Analysis complete. Implementation is sequenced below into 4 phases. Phase 1 (schema verification) is the gate — nothing else ships until the `CustomRule` struct supports the fields the drafted rules depend on.

## Key Decisions Made
- **Rules before scanners**: Ship the 25 custom-rule TOML entries first (they need only `src/detection.rs` + `src/config.rs`), defer the 6 new-scanner modules to Phase 3. Reason: custom rules are cheap, declarative, and exercise the existing engine; new scanners are structural and need trait/registration work.
- **Co-locate tests**: The 6 separate `*_tests.rs` files violate AGENTS.md. They will be merged into inline `#[cfg(test)]` modules in their parent source files. Reason: AGENTS.md is binding and the bundles explicitly call out co-location.
- **Schema extension is Phase 1**: Several drafted rules need `enabled_if_path_exists` / `disabled_if_path_exists` / `required` / `exception_pattern` / `exclude_patterns`. If these are absent from `CustomRule`, add them to `project-lint-core/src/config.rs` and `src/detection.rs` before any rule file is added.
- **Companion handoff in skills-src**: Bundle-gap findings (17 high-priority missing concepts) are NOT this repo's work — they are tracked in `~/p/gh/levonk/skills-src/.agents/handoffs/2026/08/202608051800-knowledge-bundle-gaps-for-project-lint.md`.

## Technical Context

### Stack/Tools
- Rust 2021 edition, `clap` CLI, `tree-sitter` AST, `notify` watcher, `tracing` logging, `anyhow` + `thiserror` errors, `serde` + `toml` config.
- Test deps today: `tempfile`, `assert_fs` only. Bundles require more (see Phase 2).

### Important Files
- `project-lint-core/src/config.rs` — `CustomRule` struct, profile merging. **Phase 1 target.**
- `project-lint-core/src/detection.rs` — regex engine that consumes `CustomRule`. **Phase 1 target.**
- `project-lint-core/src/scanners/file_naming.rs` — Levenshtein fuzzy matching, has `apply_fixes` tests (the only scanner that does).
- `project-lint-core/src/scanners/security.rs` — secret/weak-crypto scanning. Phase 3 extension target.
- `project-lint-core/src/scanners/typescript.rs` — TS scanner. Phase 3 extension target.
- `project-lint-core/src/hooks/mappers/{claude,windsurf,kiro}.rs` — **zero test coverage**, Phase 2 target.
- `src/commands/{lint,watch,hook,install_hook,git_hooks,policy}.rs` — command entry points.
- `tests/{integration_tests,config_tests,hook_engine_rule_tests}.rs` — integration tests.
- `Cargo.toml` (workspace) and `project-lint-core/Cargo.toml` — dev-dependency additions in Phase 2.
- `justfile` — already follows the three-flow pattern; Phase 4 adds `lint`/`ci-parity` recipes.

### Environment Notes
- Run all builds/tests via `devbox run -- cargo <cmd>` or inside `devbox shell` (AGENTS.md mandates devbox, no exceptions).
- `cargo test` is the test command. No `cargo nextest` configured yet.
- No `.github/workflows/` directory exists — CI is greenfield.

## Next Steps (Priority Order)

### Phase 1 — Schema Verification & Extension (GATE)
1. Read `project-lint-core/src/config.rs` and enumerate every field on `CustomRule` and `ProfileChecks`.
2. Compare against the field set used by the 25 drafted rules: `name`, `pattern`, `check_content`, `content_pattern`, `severity`, `message`, `triggers`, `enabled_if_path_exists`, `disabled_if_path_exists`, `required`, `exception_pattern`, `exclude_patterns`, `condition`.
3. For each missing field, add it to `CustomRule` with `#[serde(default)]` and wire consumption in `project-lint-core/src/detection.rs`. Add unit tests for each new field inline in `config.rs` and `detection.rs`.
4. Run `devbox run -- cargo test` and `devbox run -- cargo clippy --all-targets -- -D warnings`.

### Phase 2 — Test Compliance Fixes (HIGH)
1. Add dev-dependencies to `project-lint-core/Cargo.toml`: `tokio-test = "0.4"`, `serial_test = "2.0"`, `proptest = "1.4"`, `assert_cmd = "2.0"`, `predicates = "3.0"`, `criterion = { version = "0.5", features = ["html_reports"] }`. Verify each version is ≥7 days old before pinning.
2. Co-locate the 6 separate test files into inline `#[cfg(test)]` modules:
   - `project-lint-core/src/config_extended_tests.rs` → inline in `config.rs`
   - `project-lint-core/src/scanners/file_naming_tests.rs` → inline in `file_naming.rs`
   - `project-lint-core/src/hooks/engine/tests/tests_engine_tests.rs` → inline in `engine/mod.rs`
   - `project-lint-core/src/hooks/logger/tests/tests_logger_tests.rs` → inline in `logger/mod.rs`
   - `src/commands/install_hook_tests.rs` → inline in `install_hook.rs`
   - `src/commands/git_hooks_tests.rs` → inline in `git_hooks.rs`
3. Add `#[serial]` to logger tests that share state (per `testing-strategy.md`).
4. Add tests for the 3 hook event mappers (`claude.rs`, `windsurf.rs`, `kiro.rs`) — currently zero coverage. Use `serde_json::json!` to feed sample payloads and assert `ProjectLintEvent` fields.
5. Add `apply_fixes` tests for `detection.rs`, `security.rs`, `typescript.rs` (only `file_naming.rs` has them today).
6. Add error-path tests: malformed TOML config, missing config file, scanner IO errors, mapper invalid JSON.
7. Add CLI integration tests with `assert_cmd` + `predicates` for `lint`, `hook`, `policy` subcommands (exit codes, stdout/stderr).
8. Add `proptest` tests for `file_naming.rs` Levenshtein distance and `detection.rs` regex matching.
9. Create `benches/` directory with criterion benchmarks for `RuleEngine::evaluate_event` and scanner `scan` hot paths.
10. Add doc tests (`/// ```rust` blocks) to public APIs in `config.rs`, `hooks/mod.rs`, `scanners/*.rs`.
11. Run `devbox run -- cargo test` and confirm green.

### Phase 3 — Custom Rule Files & Scanner Extensions
1. Create `.config/project-lint/rules/active/rust-development.toml` with the 6 rust-development-practices rules (project-structure, no-debug-println, require-thiserror, no-unwrap-in-lib, require-rustfmt-config, require-clippy-config).
2. Create `.config/project-lint/rules/active/dev-environment.toml` with the 6 dev-environment-practices rules (require-devbox, require-direnv, require-justfile, no-makefiles, shell-strict-mode, no-npx-bunx-yarn-dlx).
3. Create `.config/project-lint/rules/active/cicd-testing.toml` with `require-quality-script` and `standard-build-targets`.
4. Create `.config/project-lint/rules/active/secrets.toml` with `no-secrets-in-shared`, `no-hardcoded-credentials`, `no-weak-crypto`, `no-unsafe-c-functions`.
5. Create `.config/project-lint/rules/active/container.toml` with `pin-image-digests`, `no-copy-dot`, `require-non-root-user`.
6. Create `.config/project-lint/rules/active/upstream.toml` with `no-main-commits` (triggers `pre_write_code`), `no-user-identity-leak`.
7. Create `.config/project-lint/rules/active/typescript-monorepo.toml` with `no-ambiguous-ts-extensions`, `no-bare-path-aliases`, `require-pnpm-workspace`.
8. Create `.config/project-lint/rules/active/software-architecture.toml` with `require-readme`.
9. Extend `file_naming.rs` with the `[rust_file_naming]` and `[dev_environment_files]` config sections (required_files / forbidden_files / test_naming_pattern).
10. Extend `security.rs` with `[rust_security]`, `[vault_security]`, `[dockerfile_security]` config sections.
11. Extend `typescript.rs` with `[typescript_monorepo]` config section (catalog mode, path aliases, extensions).
12. Extend `detection.rs` with `[package_manager_enforcement]` config section.
13. Register any new scanner modules (`rust_conventions.rs`, `dev_environment.rs`, `ci_cd_parity.rs`, `dockerfile_lint.rs`, `typescript_monorepo.rs`, `vault_security.rs`) in `project-lint-core/src/lib.rs` and integrate into `src/commands/lint.rs::run`. Defer these to Phase 3b if Phase 3a (rule files + extensions) is large.

### Phase 4 — Pre-Commit & CI Parity
1. Create `scripts/run-quality-checks.sh` with `FAST_MODE` / `FULL_MODE` env toggles, pinned Docker image, running `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo audit`. Follow `shared-quality-scripts.md`.
2. Add a `just lint` recipe and a `just ci-parity` recipe that invoke the script.
3. Create `.github/workflows/ci.yml` with matrix (stable/beta/nightly × linux/macos/windows), calling `scripts/run-quality-checks.sh` with `FULL_MODE=1`. Add `cargo audit` and `cargo-deny` steps. Follow `quality-gates.md`.
4. Add a pre-commit hook (`.git/hooks/pre-commit` or `lefthook.yml`) that calls the script with `FAST_MODE=1`.
5. Run `devbox run -- cargo test` after every phase to confirm no regressions.

## Success Criteria
- ✅ `devbox run -- cargo test` passes with all new tests (target: +50 tests covering mappers, apply_fixes, error paths, CLI, proptest, doc tests).
- ✅ `devbox run -- cargo clippy --all-targets --all-features -- -D warnings` is clean.
- ✅ `devbox run -- cargo fmt --all -- --check` is clean.
- ✅ All 6 previously-separate test files are co-located inline (no `*_tests.rs` siblings remain).
- ✅ `.config/project-lint/rules/active/*.toml` contains 7 rule files with 25 custom rules, all loadable by `Config::load` without errors.
- ✅ `scripts/run-quality-checks.sh` exists, is executable, and runs locally via `just ci-parity`.
- ✅ `.github/workflows/ci.yml` exists with a 3×3 matrix and `cargo audit` step.
- ✅ Hook event mappers (`claude`, `windsurf`, `kiro`) have ≥3 tests each.
- ✅ `benches/` directory has ≥2 criterion benchmarks compiling.

## Open Questions/Blockers
- **Q1**: Does `CustomRule` already support `enabled_if_path_exists` / `disabled_if_path_exists` / `required` / `exception_pattern` / `exclude_patterns`? If not, is there appetite to extend the schema, or should the rules be rewritten to use only existing fields? — Impact: blocks Phase 1.
- **Q2**: Are the 6 new scanner modules (`rust_conventions`, `dev_environment`, `ci_cd_parity`, `dockerfile_lint`, `typescript_monorepo`, `vault_security`) in scope for this repo, or should they live in a separate `project-lint-scanners` crate? — Impact: affects Phase 3b structure.
- **Q3**: Should the rule files live under `.config/project-lint/rules/active/` (this repo) or be shipped as a default profile inside the binary? — Impact: affects how users opt in.
- **Q4**: Is `cargo audit` acceptable in CI given it requires an advisory database fetch? — Impact: affects CI runtime and offline support.

## Do Not
- Do NOT edit `~/p/gh/levonk/skills-src/build/current/knowledge/*` directly — those are read-only rendered outputs. Bundle-gap fixes are tracked in the companion handoff in skills-src and must edit `src/current/knowledge/*.tmpl` source files.
- Do NOT add the 6 dev-dependencies without verifying each version is ≥7 days old (AGENTS.md supply-chain rule).
- Do NOT run `cargo` commands outside `devbox run --` (AGENTS.md mandates devbox, no exceptions — a prefer-rule gets skipped).
- Do NOT add AI attribution boilerplate to commits or files (global rule in `~/.config/devin/AGENTS.md`).
- Do NOT weaken the AGENTS.md co-location convention — the 6 separate test files must move inline, not the convention relaxed.
- Do NOT ship rule TOML files before Phase 1 schema work confirms the fields exist.

## Suggested Skills
- **handoff** — already invoked; this document is its output. Use it again when capturing context after Phase 1 completes.
- **git-repository-management** — for staging and committing each phase as a separate commit (rollback-safe ordering).
- **unit-test-writing** — for the Roy Osherove-style test structure the bundles require (readable, maintainable, trustworthy).
- **code-quality-validation** — for the `scripts/run-quality-checks.sh` content (lint/test/security scan composite).
- **knowledge-bundle-lifecycle** — only if you need to query a bundle for clarification on a rule; the bundle edits themselves belong to the skills-src handoff.
- **surgical-config** — for the `Cargo.toml` dev-dependency additions (non-destructive, structure-preserving edits).

## Additional Context

### Source Reports
The three subagent reports that produced this handoff are saved as overflow files (read them if you need the raw findings):
- Rule opportunities: `/var/folders/9t/pzrk6_p92sncb29dczlpjh9c0000gn/T/devin-overflows-501/3924d58a/content.txt`
- Test compliance audit: `/var/folders/9t/pzrk6_p92sncb29dczlpjh9c0000gn/T/devin-overflows-501/1163afea/content.txt`
- Bundle gap analysis: `/var/folders/9t/pzrk6_p92sncb29dczlpjh9c0000gn/T/devin-overflows-501/4b185829/content.txt`

These temp files may be GC'd; the synthesized findings are captured in this handoff and the skills-src companion handoff.

### Companion Handoff
Bundle-gap findings (17 high-priority missing concepts in the knowledge bundles) are tracked separately in:
`~/p/gh/levonk/skills-src/.agents/handoffs/2026/08/202608051800-knowledge-bundle-gaps-for-project-lint.md`

That handoff covers edits to `src/current/knowledge/*.tmpl` source files (tree-sitter AST queries, clap CLI patterns, Rust CI tooling, regex DoS prevention, path traversal, plugin/scanner registration, rule engine design, etc.). Do not duplicate that work here.

### Rule-Count Summary
- 25 custom rules across 7 TOML files (Phase 3a)
- 9 existing-scanner extensions (Phase 3a)
- 6 new-scanner proposals (Phase 3b, optional)
- 12 bundles reviewed and marked out-of-scope (career, resume, ai-primitives, api-auth, cloud, data-eng, frontend, networking, java, python, docs-diagrams, ste, web-resource-catalog)

### Test-Count Summary
- 98 existing tests across 23 files
- 6 files violate co-location (to be merged inline)
- 3 hook mappers with zero coverage (to be tested)
- 3 of 4 `apply_fixes` methods untested (to be tested)
- 6 dev-dependencies to add
- 0 CI workflows today (to be created)
