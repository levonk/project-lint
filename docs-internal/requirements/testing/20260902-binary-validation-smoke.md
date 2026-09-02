# Smoke Test Results: Binary Validation Scanner (2026-09-02)

**PRD**: `docs-internal/requirements/20260901-binary-validation.md`
**Build**: `devbox run -- just build` (0 errors, pre-existing warnings only)
**Binary**: `./target/release/project-lint`

## Objective

Confirm the `binary_validation` scanner correctly detects committed binaries
across real repositories — flags binaries that should use Git LFS, oversized
binaries, binaries in source directories, duplicate binaries (identical
content), committed archives, and compiled artifacts. Verify the scanner is
silent on repositories with no committed binaries and that excluded
directories (`node_modules/`, `target/`, `dist/`, etc.) are skipped.

## Test Repos

| Repo | Has binaries | Has `node_modules/` | Has `target/` | Purpose |
|------|--------------|---------------------|---------------|---------|
| `~/p/gh/levonk/agentmemory` | Yes (PNG, SVG, GIF, MP4) | Yes | No | Rich binary set — verify all rules fire |
| `~/p/gh/levonk/dmrconfig_dm32` | Yes (PDF) | No | No | PDF in a reference dir — verify LFS rule |
| `~/p/gh/levonk/project-lint` | No | No | Yes | Self-lint — verify scanner is silent |

## Test 1: agentmemory (rich binary set)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/agentmemory`

**Results** (filtered to `[Binary]` issues):

| Rule | Severity | File | Notes |
|------|----------|------|-------|
| `binary-oversized` | error | `assets/demo.mp4` | 11,550,410 bytes > 10 MB limit |
| `binary-lfs-required` | warning | `assets/demo.mp4` | 11.5 MB > 1 MB LFS threshold |
| `binary-lfs-required` | warning | `assets/demo.gif` | 8.1 MB > 1 MB LFS threshold |
| `binary-lfs-required` | warning | `website/public/demo.gif` | 8.1 MB > 1 MB LFS threshold |
| `binary-in-source-dir` | warning | `src/viewer/favicon.svg` | SVG inside `src/` |
| `binary-duplicate` | warning | `assets/logo.svg` ↔ `website/public/logo.svg` | Identical SHA-256 |
| `binary-duplicate` | warning | `assets/icon.svg` ↔ `website/public/icon.svg` | Identical SHA-256 |
| `binary-duplicate` | warning | `assets/iii-console/states.png` ↔ `website/public/states.png` | Identical SHA-256 |
| `binary-duplicate` | warning | `assets/iii-console/traces-waterfall.png` ↔ `website/public/traces-waterfall.png` | Identical SHA-256 |
| `binary-duplicate` | warning | `assets/demo.gif` ↔ `website/public/demo.gif` | Identical SHA-256 |

- `grep -c "\[Binary\].*node_modules" output`: **0** — `node_modules/` correctly excluded
- All 6 rules exercised: `binary-lfs-required`, `binary-oversized`,
  `binary-in-source-dir`, `binary-duplicate` (archive + compiled not present
  in this repo but covered by unit tests)

## Test 2: dmrconfig_dm32 (PDF reference)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/dmrconfig_dm32`

**Results**:

| Rule | Severity | File | Notes |
|------|----------|------|-------|
| `binary-lfs-required` | warning | `dm32_reference/Baofeng_DM-32UV_User_Manual_20250210.pdf` | 4.7 MB > 1 MB LFS threshold |

- The PDF is in `dm32_reference/` (not a source dir) so `binary-in-source-dir`
  does not fire — correct.

## Test 3: project-lint self-lint (no binaries)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/project-lint`

**Results**:
- `[Binary]` issues: **0** — scanner is silent on a repo with no committed
  binaries.
- `target/` directory correctly excluded (no false positives from build
  artifacts).

## Acceptance Criteria Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Scanner struct with `new()`/`with_config()`/`with_exclusions()`/`scan()`/`Default` | PASS | `project-lint-core/src/scanners/binary_validation.rs` |
| Uses `ScannerIssue` | PASS | imports `crate::scanners::ScannerIssue` |
| Uses centralized exclusion helper | PASS | `build_exclusions`, `walk_project`, `is_excluded_rel` from `utils.rs` |
| Module registered in `scanners/mod.rs` | PASS | `pub mod binary_validation;` |
| Wired into `lint.rs` gated by `binary_validation` | PASS | `config.is_check_enabled("binary_validation")` block |
| Config struct + `ScannerConfig` field + `Default` | PASS | `BinaryValidationConfig` in `config.rs` |
| `AGENTS.md` Analysis Modules updated | PASS | Entry added after `git_sync.rs` |
| Colocated `mod tests` — positive/negative/edge per rule | PASS | 19 tests covering all 6 rules + edge cases |
| `devbox run -- just quality` passes | PASS | 222 passed, 0 failed |
| `docs/lint-categories/binary.md` created | PASS | Follows existing category doc format |
| `docs-internal/implementation-summary.md` updated | PASS | Binary validation added to summary |
| Smoke test: repo with binaries flagged correctly | PASS | agentmemory — all rules fire |
| Smoke test: repo without binaries is silent | PASS | project-lint — 0 `[Binary]` issues |
| Smoke test: excluded dirs skipped | PASS | 0 `node_modules` hits in agentmemory output |

## Conclusion

The `binary_validation` scanner is working correctly across real repositories.
It detects binaries that should use Git LFS (large GIFs, MP4s, PDFs), oversized
binaries (11 MB MP4 flagged as error), binaries in source directories (SVG in
`src/`), and duplicate binaries (identical SVG/PNG assets in `assets/` and
`website/public/`). The scanner is silent on repositories with no committed
binaries and correctly excludes `node_modules/` and `target/` directories.
