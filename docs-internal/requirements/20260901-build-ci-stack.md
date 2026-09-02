# PRD: Build/CI Stack (GitHub Workflows, Dependabot, Justfile, Makefile, Process-Compose)

**Date**: 2026-09-01
**Status**: proposed
**Scope**: New scanners for CI/CD and build system configuration files:
GitHub Actions workflow content validation, `dependabot.yml`
validation, `justfile` content validation (beyond target names),
`Makefile` content validation, and `process-compose.yaml` validation.

## Problem

The existing `ci_cd_parity` scanner checks that `.github/workflows/`
exists and that `justfile` defines standard target names (`clean`,
`build`, `test`, `lint`, `typecheck`, `fmt`). But it never reads
workflow file content or validates CI configuration. The scan data
shows:

- 45+ `.github/workflows/*.yml` files — no content validation
- 9 `dependabot.yml` files — no validation
- 29 `justfile` files — only target names checked, not content
- 26 `Makefile` files — flagged as forbidden but not content-validated
- 9 `process-compose.yaml` files — no scanner at all

## File Types Covered

| File type | Count | Scanner |
|-----------|-------|---------|
| `.github/workflows/*.yml` | ~45 | github_workflow |
| `.github/dependabot.yml` | ~9 | dependabot |
| `justfile` / `Justfile` | ~29 | justfile_content |
| `Makefile` | ~26 | makefile_content |
| `process-compose.yaml` / `.yml` | ~9 | process_compose |

## Rules

### github_workflow (check name: `github_workflow`) — NEW SCANNER

#### Security rules
- [ ] `workflow-permissions-block` — Workflows should have an explicit `permissions:` block (not rely on default token permissions). **Severity: warning.** Auto-fixable: no.
- [ ] `workflow-permissions-minimal` — `permissions:` should be set to `contents: read` or more restrictive, not `contents: write` unless needed. **Severity: warning.** Auto-fixable: no.
- [ ] `workflow-no-pull-request-target` — Workflows must not use `on: pull_request_target` (security risk — runs with secrets on fork PRs). **Severity: error.** Auto-fixable: no.
- [ ] `workflow-pinned-actions` — GitHub Actions should be pinned by SHA (`uses: actions/checkout@<sha>`), not by tag (`@v4`). **Severity: warning.** Auto-fixable: no.
- [ ] `workflow-no-inject-secrets` — Workflows must not inject secrets into environment variables that could be logged (`env: TOKEN: ${{ secrets.TOKEN }}` in steps that echo output). **Severity: error.** Auto-fixable: no.
- [ ] `workflow-no-sudo` — Workflows should not use `sudo` in steps (GitHub runners already have appropriate permissions). **Severity: info.** Auto-fixable: no.

#### Quality rules
- [ ] `workflow-runs-on-valid` — `runs-on:` should be a valid runner label or matrix. **Severity: warning.** Auto-fixable: no.
- [ ] `workflow-concurrency` — Workflows should define `concurrency:` to cancel stale runs. **Severity: info.** Auto-fixable: no.
- [ ] `workflow-timeout` — Workflows should define `timeout-minutes:` to prevent hung runs. **Severity: warning.** Auto-fixable: no.
- [ ] `workflow-uses-devbox` — CI workflows should use `devbox run --` for build/test commands, not raw commands. **Severity: warning.** Auto-fixable: no. **Note**: Configurable — set `require_devbox = false` for non-devbox projects.

### dependabot (check name: `dependabot`) — NEW SCANNER

- [ ] `dependabot-ecosystem-coverage` — `dependabot.yml` should cover all package ecosystems used by the project (cargo, npm, github-actions, etc.). **Severity: warning.** Auto-fixable: no.
- [ ] `dependabot-schedule` — Each ecosystem entry should have a `schedule.interval` (daily, weekly, monthly). **Severity: error.** Auto-fixable: no.
- [ ] `dependabot-group-config` — Entries should use `groups` to batch updates (reduces PR noise). **Severity: info.** Auto-fixable: no.
- [ ] `dependabot-assignees-reviewers` — Entries should have `assignees` or `reviewers` configured. **Severity: info.** Auto-fixable: no.
- [ ] `dependabot-actions-ecosystem` — If the project uses GitHub Actions (`.github/workflows/`), `dependabot.yml` should have an entry for `github-actions` ecosystem. **Severity: warning.** Auto-fixable: no.

### justfile_content (check name: `justfile_content`) — NEW SCANNER

#### Structure rules
- [ ] `justfile-quality-target` — `justfile` should define a `quality` target (fmt check + clippy + tests). **Severity: error.** Auto-fixable: no. **Note**: This overlaps with `ci_cd_parity` scanner — consolidate by moving target-name checks here.
- [ ] `justfile-quality-full-target` — `justfile` should define a `quality-full` target (quality + doc tests + bench compile + audit). **Severity: warning.** Auto-fixable: no.
- [ ] `justfile-ci-target` — `justfile` should define a `ci` target that maps to `quality-full`. **Severity: warning.** Auto-fixable: no.
- [ ] `justfile-bootstrap-target` — `justfile` should define a `bootstrap` target for first-time setup. **Severity: info.** Auto-fixable: no.

#### Content rules
- [ ] `justfile-uses-devbox-run` — Targets that run build tools (`cargo`, `pnpm`, `go`, etc.) should prefix with `devbox run --` or be inside a devbox shell. **Severity: warning.** Auto-fixable: no.
- [ ] `justfile-no-absolute-paths` — `justfile` should not contain hardcoded absolute paths. Use `justfile_directory()`, `env_var()`, or relative paths. **Severity: error.** Auto-fixable: no.
- [ ] `justfile-no-npx-bunx-yarn` — `justfile` must not contain `npx`, `bunx`, or `yarn` commands. Use `pnpm dlx` or `pnpm exec`. **Severity: error.** Auto-fixable: no.
- [ ] `justfile-no-raw-cargo` — In devbox projects, `justfile` should not call `cargo` directly — use `devbox run -- cargo` to ensure the correct toolchain. **Severity: warning.** Auto-fixable: no. **Note**: Configurable via `require_devbox_wrapper`.

### makefile_content (check name: `makefile_content`) — NEW SCANNER

- [ ] `makefile-forbidden` — `Makefile` is present and should be migrated to `justfile`. **Severity: warning.** Auto-fixable: no. **Note**: This overlaps with `dev_environment` scanner — consolidate.
- [ ] `makefile-no-absolute-paths` — `Makefile` should not contain hardcoded absolute paths. **Severity: error.** Auto-fixable: no.
- [ ] `makefile-no-cd-absolute` — `Makefile` should not use `cd /absolute/path` in rules. **Severity: error.** Auto-fixable: no.
- [ ] `makefile-uses-just-delegation` — If `Makefile` must exist (for upstream compatibility), it should delegate to `just`: `build: ; just build`. **Severity: info.** Auto-fixable: no.

### process_compose (check name: `process_compose`) — NEW SCANNER

- [ ] `process-compose-valid-commands` — Each process in `process-compose.yaml` should have a valid `command` field. **Severity: error.** Auto-fixable: no.
- [ ] `process-compose-health-check` — Long-running processes should define `health_check`. **Severity: warning.** Auto-fixable: no.
- [ ] `process-compose-restart-policy` — Processes should define `restart_policy` (unless they are one-shot). **Severity: info.** Auto-fixable: no.
- [ ] `process-compose-no-absolute-paths` — Commands should not use absolute paths. **Severity: warning.** Auto-fixable: no.
- [ ] `process-compose-uses-devbox` — Commands should use `devbox run --` prefix in devbox projects. **Severity: warning.** Auto-fixable: no.

## Implementation

All scanners parse their respective files as text or YAML and check
patterns. GitHub workflows and dependabot are YAML — use `serde_yaml`
for parsing. Justfile and Makefile are text — use regex/line parsing.
Process-compose is YAML — use `serde_yaml`.

## Configuration

```toml
[scanner_config.github_workflow]
require_permissions = true
require_pinned_actions = true
require_timeout = true
require_devbox = true  # set false for non-devbox projects
forbid_pull_request_target = true

[scanner_config.dependabot]
check_ecosystem_coverage = true
require_group_config = false

[scanner_config.justfile_content]
require_devbox_wrapper = true
forbidden_commands = ["npx", "bunx", "yarn"]
required_targets = ["quality", "quality-full", "ci"]

[scanner_config.makefile_content]
require_just_delegation = false

[scanner_config.process_compose]
require_health_check = true
require_devbox = true
```

## Acceptance Criteria

- [ ] All five scanners exist with `scan()` returning `Vec<ScannerIssue>`
- [ ] All five registered in `mod.rs`, wired in `lint.rs`, config in `config.rs`, documented in `AGENTS.md`
- [ ] `github_workflow` scanner parses YAML workflows and checks permissions, action pinning, pull_request_target
- [ ] `justfile_content` scanner checks for devbox wrapper, forbidden commands, required targets
- [ ] Consolidation plan: `ci_cd_parity` target-name checks should be moved to `justfile_content`, `dev_environment` Makefile check should be moved to `makefile_content` (or cross-reference)
- [ ] All scanners use centralized exclusion list
- [ ] Tests for each rule
- [ ] Smoke test: silent on repos without these files
- [ ] Smoke test: fires on `project-lint` (has justfile + workflows)
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **GitLab CI** — `.gitlab-ci.yml` not covered (no GitLab repos found in scan). Future scanner.
- **CircleCI** — `.circleci/config.yml` not covered. Future scanner.
- **Jenkins** — `Jenkinsfile` not covered. Future scanner.
- **Drone CI** — `.drone.yml` not covered. Future scanner.
- **Woodpecker CI** — `.woodpecker.yml` not covered. Future scanner.
- **Pre-commit hooks** — `.pre-commit-config.yaml` not covered. Future scanner.
- **Renovate bot** — `renovate.json` not covered (separate from dependabot). Future scanner.

## Dependencies

- **Centralized exclusion list** — must not scan `node_modules/`, `target/`, etc.
- **`serde_yaml` crate** — for parsing workflow YAML, dependabot YAML, process-compose YAML.
