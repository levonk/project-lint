# Smoke Test Results: Centralized Exclusion List (2026-09-02)

**PRD**: `docs-internal/requirements/20260901-centralized-exclusion-list.md`
**Build**: `cargo build --release` (0 errors, 61 pre-existing warnings)
**Binary**: `./target/release/project-lint`

## Objective

Confirm that all WalkDir-based scanners use the centralized exclusion list and
do NOT scan `node_modules/`, `target/`, `dist/`, `build/`, `.git/`, or other
build artifacts. Verify scanners are silent on non-matching repos and fire
correctly on matching repos.

## Test Repos

| Repo | Has `node_modules/` | Has `target/` | Has Dockerfiles | Purpose |
|------|---------------------|---------------|-----------------|---------|
| `~/p/gh/levonk/project-lint` | No | Yes | Yes (`temp_smoke_test/`) | Self-lint — verify `target/` excluded |
| `~/p/gh/levonk/buzz` | Yes | Yes (Rust crates) | Yes | Mixed Rust+TS repo — verify `node_modules/` excluded |

## Test 1: project-lint self-lint

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/project-lint`

**Results**:
- Scanners fired: `[Rust]`, `[Docker]`, `[Vault]`, `[MagicNum]`, `[GitSync]`, `[Naming]`
- `[Rust]` issues: all from `project-lint-core/src/` and `src/` — **zero** from `target/`
- `[Docker]` issues: from `temp_smoke_test/iac-stack/Dockerfile` — **zero** from `target/`
- `[Vault]` issues: from `project-lint-core/src/scanners/security.rs` and `vault_security.rs` — **zero** from `target/`
- `[MagicNum]` issues: from `temp_smoke_test/iac-stack/docker-compose.yml` — **zero** from `target/`
- `grep -c "node_modules" output`: **0**
- `grep -c "\[Rust\].*target/" output`: **0**
- `grep -c "\[Docker\].*target/" output`: **0**
- `grep -c "\[Vault\].*target/" output`: **0**

**Note**: The `worktree-isolation-enforcer` modular rule fires on `target/` files
because it uses a separate WalkDir traversal in `lint.rs` that has NOT been
migrated to the centralized exclusion list yet. This is expected — the
worktree-isolation rule is a modular rule, not one of the 6 scanners in scope
for this PRD. It will be addressed when the modular-rule walker is migrated.

## Test 2: buzz repo (has `node_modules/`)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/buzz`

**Results**:
- Scanners fired: `[Rust]`, `[MagicNum]`, `[SkillMD]`, `[GitSync]`
- `[Rust]` issues: all from `crates/buzz-db/src/`, `crates/buzz-acp/src/` — **zero** from `node_modules/`
- `[MagicNum]` issues: from `.github/workflows/*.yml`, `docker-compose.yml`, `prometheus.yml` — **zero** from `node_modules/`
- `[SkillMD]` issues: from `examples/meadow-core/skills/` — **zero** from `node_modules/`
- `grep -c "node_modules" output`: **0**

## Acceptance Criteria Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| `is_excluded_dir()` exists in `utils.rs` | PASS | `is_excluded_rel()` + `is_excluded_entry()` in `project-lint-core/src/utils.rs` |
| `walk_project()` helper exists | PASS | `walk_project()` in `project-lint-core/src/utils.rs` |
| All existing WalkDir scanners updated | PASS | 6 scanners migrated: `rust_conventions`, `dockerfile_lint`, `skill_markdown`, `magic_numbers`, `vault_security`, `file_naming` |
| Unit tests verify each excluded dir | PASS | 15 unit tests in `utils::tests` |
| Unit test: `extra_excludes` applied | PASS | `walk_project_respects_extra_excludes` |
| Unit test: `allow_vendor = true` | PASS | `build_exclusions_omits_vendor_when_allowed` + `is_excluded_rel_respects_vendor_toggle` |
| Smoke: `node_modules/` not scanned | PASS | 0 `node_modules` hits in buzz repo output |
| Smoke: `target/` not scanned by migrated scanners | PASS | 0 `target/` hits in `[Rust]`/`[Docker]`/`[Vault]` output |
| `cargo test --workspace` passes | PASS | 202 passed, 0 failed |
| `cargo build --release` passes | PASS | 0 errors |

## Out-of-Scope Observations

- The `worktree-isolation-enforcer` modular rule still walks `target/` because
  it uses its own WalkDir traversal in `lint.rs` (lines 401, 523, 570, 656, 843,
  883, 936, 1037). These are not WalkDir-based *scanners* — they are
  modular-rule processing loops. Migrating them to `walk_project()` is a
  separate task outside this PRD's scope.
- `submodule_integrity.rs` uses `git2` tree walking, not `WalkDir`, so it is
  correctly excluded from this migration.

## Conclusion

The centralized exclusion list is working correctly. All 6 WalkDir-based
scanners now use `walk_project()` and `is_excluded_rel()` from
`project-lint-core/src/utils.rs`. Build artifacts and dependency directories
are correctly excluded from all migrated scanners.
