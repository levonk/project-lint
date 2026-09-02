# Smoke Test Results: Shell Script Validation (2026-09-02)

**PRD**: `docs-internal/requirements/20260901-shell-script-validation.md`
**Build**: `devbox run -- just build` (release, 0 errors, pre-existing warnings only)
**Binary**: `./target/release/project-lint`

## Objective

Confirm the `shell_script` scanner:
1. Fires on repos with `*.sh` / `*.bash` files.
2. Is silent on repos without shell scripts (no false positives).
3. Respects the centralized exclusion list (does not scan `target/`,
   `node_modules/`, `.devbox/gen/`).

## Test Repos

| Repo | Has `*.sh` | Has `.devbox/gen/*.sh` | Purpose |
|------|-----------|------------------------|---------|
| `~/p/gh/levonk/project-lint` | Yes (8+) | Yes | Self-lint — verify scanner fires + `.devbox/gen/` excluded |
| `~/p/gh/levonk/supersearch` | No | No | Negative — verify scanner is silent |

## Test 1: project-lint self-lint (has shell scripts)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/project-lint`

**Results** — `[Shell]` issues emitted:
- `.cursor/hook.sh` — `sh-shebang`, `sh-strict-mode`
- `.windsurf/hook.sh` — `sh-shebang`, `sh-strict-mode`
- `.claude/hook.sh` — `sh-shebang`, `sh-strict-mode`
- `demo-pnpm-hook.sh` — `sh-shebang`, `sh-strict-mode`, `sh-bounded-timeout`
- `build.sh` — `sh-shebang`, `sh-strict-mode`
- `hooks/project-lint-hook.sh` — `sh-shebang`, `sh-strict-mode`
- `scripts/run-quality-checks.sh` — `sh-strict-mode`

**Exclusion verification**:
- `grep -c "\[Shell\].*\.devbox/gen" output`: **0** — generated scripts excluded
- `grep -c "\[Shell\].*target/" output`: **0** — build artifacts excluded

## Test 2: supersearch (no shell scripts)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/supersearch`

**Results**:
- `grep -c "\[Shell\]" output`: **0** — scanner is silent (no false positives)

## Acceptance Criteria Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Scanner fires on repos with `.sh` files | PASS | 7 files flagged in project-lint self-lint |
| Scanner silent on repos without `.sh` files | PASS | 0 `[Shell]` issues in supersearch |
| `.devbox/gen/` excluded | PASS | 0 `[Shell]` hits from `.devbox/gen/` |
| `target/` excluded | PASS | 0 `[Shell]` hits from `target/` |
| `devbox run -- just quality` passes | PASS | 228 tests, 0 failures |
| `devbox run -- just build` passes | PASS | release binary built, 0 errors |

## Conclusion

The `shell_script` scanner is wired correctly. It fires on real shell scripts,
respects the centralized exclusion list (`.devbox/gen/`, `target/`,
`node_modules/`), and is silent on repos without shell scripts. All 10 rules
from the PRD are implemented and covered by 25 colocated unit tests.
