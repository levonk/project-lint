# Build/CI Stack Rules

Build/CI stack rules validate GitHub Actions workflows, Dependabot configuration, justfile content, Makefile content, and process-compose files for security, quality, and consistency.

## Overview

Build/CI rules help identify:
- Missing or overly permissive workflow permissions
- Unpinned GitHub Actions (security risk)
- Dangerous workflow triggers (`pull_request_target`)
- Missing workflow timeouts and concurrency settings
- Dependabot ecosystem coverage gaps
- Missing required justfile targets (`quality`, `quality-full`, `ci`)
- Forbidden commands in justfiles (`npx`, `bunx`, `yarn`)
- Makefile presence (should migrate to justfile)
- Process-compose health checks and restart policies

## Scanners

### github_workflow (check name: `github_workflow`)

Validates `.github/workflows/*.yml` files for security and quality.

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `workflow-permissions-block` | warning | Workflow should have explicit `permissions:` block |
| `workflow-permissions-minimal` | warning | `permissions:` should be `contents: read` or more restrictive |
| `workflow-no-pull-request-target` | error | Must not use `on: pull_request_target` (security risk) |
| `workflow-pinned-actions` | warning | Actions should be pinned by SHA, not by tag |
| `workflow-no-inject-secrets` | error | Must not inject secrets into env vars that could be logged |
| `workflow-no-sudo` | info | Should not use `sudo` in steps |
| `workflow-runs-on-valid` | warning | `runs-on:` should be a valid runner label or matrix |
| `workflow-concurrency` | info | Should define `concurrency:` to cancel stale runs |
| `workflow-timeout` | warning | Should define `timeout-minutes:` to prevent hung runs |
| `workflow-uses-devbox` | warning | CI should use `devbox run --` for build/test commands |

#### Configuration

```toml
[scanner_config.github_workflow]
require_permissions = true
require_pinned_actions = true
require_timeout = true
require_devbox = true
forbid_pull_request_target = true
```

### dependabot (check name: `dependabot`)

Validates `.github/dependabot.yml` for ecosystem coverage and update configuration.

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `dependabot-ecosystem-coverage` | warning | Should cover all package ecosystems used by the project |
| `dependabot-schedule` | error | Each entry should have `schedule.interval` |
| `dependabot-group-config` | info | Entries should use `groups` to batch updates |
| `dependabot-assignees-reviewers` | info | Entries should have `assignees` or `reviewers` |
| `dependabot-actions-ecosystem` | warning | Should have `github-actions` entry if workflows exist |

#### Configuration

```toml
[scanner_config.dependabot]
check_ecosystem_coverage = true
require_group_config = false
```

### justfile_content (check name: `justfile_content`)

Validates `justfile` / `Justfile` content for required targets and best practices.

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `justfile-quality-target` | error | Should define `quality` target (fmt + clippy + tests) |
| `justfile-quality-full-target` | warning | Should define `quality-full` target |
| `justfile-ci-target` | warning | Should define `ci` target mapping to `quality-full` |
| `justfile-bootstrap-target` | info | Should define `bootstrap` target for first-time setup |
| `justfile-uses-devbox-run` | warning | Build tool commands should use `devbox run --` prefix |
| `justfile-no-absolute-paths` | error | Should not contain hardcoded absolute paths |
| `justfile-no-npx-bunx-yarn` | error | Must not contain `npx`, `bunx`, or `yarn` commands |
| `justfile-no-raw-cargo` | warning | Should not call `cargo` directly in devbox projects |

#### Configuration

```toml
[scanner_config.justfile_content]
require_devbox_wrapper = true
forbidden_commands = ["npx", "bunx", "yarn"]
required_targets = ["quality", "quality-full", "ci"]
```

### makefile_content (check name: `makefile_content`)

Validates `Makefile` content — flags presence and checks for anti-patterns.

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `makefile-forbidden` | warning | Makefile present; should be migrated to justfile |
| `makefile-no-absolute-paths` | error | Should not contain hardcoded absolute paths |
| `makefile-no-cd-absolute` | error | Should not use `cd /absolute/path` in rules |
| `makefile-uses-just-delegation` | info | Should delegate to `just` if Makefile must exist |

#### Configuration

```toml
[scanner_config.makefile_content]
require_just_delegation = false
```

### process_compose (check name: `process_compose`)

Validates `process-compose.yaml` / `.yml` for process configuration quality.

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `process-compose-valid-commands` | error | Each process should have a valid `command` field |
| `process-compose-health-check` | warning | Long-running processes should define `health_check` |
| `process-compose-restart-policy` | info | Processes should define `restart_policy` |
| `process-compose-no-absolute-paths` | warning | Commands should not use absolute paths |
| `process-compose-uses-devbox` | warning | Commands should use `devbox run --` prefix |

#### Configuration

```toml
[scanner_config.process_compose]
require_health_check = true
require_devbox = true
```

## Enabling Scanners

Add the check names to your configuration:

```toml
[rules]
enabled_checks = [
    "github_workflow",
    "dependabot",
    "justfile_content",
    "makefile_content",
    "process_compose",
]
```

Or enable via a profile in `.config/project-lint/rules/profiles/build-ci.toml`.

## Out of Scope

- GitLab CI (`.gitlab-ci.yml`) — future scanner
- CircleCI (`.circleci/config.yml`) — future scanner
- Jenkins (`Jenkinsfile`) — future scanner
- Drone CI (`.drone.yml`) — future scanner
- Woodpecker CI (`.woodpecker.yml`) — future scanner
- Pre-commit hooks (`.pre-commit-config.yaml`) — future scanner
- Renovate bot (`renovate.json`) — future scanner
