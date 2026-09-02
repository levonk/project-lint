# PRD: Shell Script Validation

**Date**: 2026-09-01
**Status**: implemented
**Scope**: New scanner for shell script (`.sh`) content validation.
Enforces shell scripting best practices from the
`dev-environment-practices/shell-scripting-best-practices` knowledge
bundle. 561 `.sh` files found across repos.

## File Types Covered

| File type | Count | Scanner |
|-----------|-------|---------|
| `*.sh` | ~561 | shell_script |
| `*.bash` | ~10 | shell_script |

## Rules

- [x] `sh-shebang` — Executable scripts must start with `#!/usr/bin/env bash` (not `#!/bin/bash` or `#!/bin/sh`). **Severity: warning.** Auto-fixable: yes.
- [x] `sh-strict-mode` — Scripts must have `set -euo pipefail` after shebang. **Severity: warning.** Auto-fixable: yes.
- [x] `sh-exec-final-command` — When final action is a long-lived command, use `exec`. **Severity: info.** Auto-fixable: yes.
- [x] `sh-path-addition-guard` — PATH additions must be guarded against duplicates (`case ":$PATH:" in *":$dir:"*)`). **Severity: warning.** Auto-fixable: yes.
- [x] `sh-git-cleanliness-gate` — Destructive scripts must check for dirty git state. **Severity: warning.** Auto-fixable: no.
- [x] `sh-dry-run-first` — Destructive steps must have dry-run capability. **Severity: warning.** Auto-fixable: no.
- [x] `sh-bounded-timeout` — Long-running operations should use `timeout`. **Severity: info.** Auto-fixable: yes.
- [x] `sh-no-hardcoded-home` — No `/Users/<user>/`, `/home/<user>/`, `C:\Users\` paths. **Severity: warning.** Auto-fixable: no.
- [x] `sh-no-npx-bunx-yarn` — No `npx`, `bunx`, `yarn dlx` commands (use `pnpm dlx`). **Severity: error.** Auto-fixable: yes. **Note**: Container exception does not apply to .sh files on host.
- [x] `sh-uses-devbox-run` — In devbox projects, scripts running build tools should use `devbox run --`. **Severity: warning.** Auto-fixable: no.

## Configuration

```toml
[scanner_config.shell_script]
require_strict_mode = true
require_shebang = true
forbid_hardcoded_home = true
forbidden_commands = ["npx", "bunx", "yarn dlx"]
require_devbox_run = true  # only in projects with devbox.json
```

## Acceptance Criteria

- [x] `ShellScriptScanner` exists with `scan()` returning `Vec<ScannerIssue>`
- [x] Registered in `mod.rs`, wired in `lint.rs`, config in `config.rs`, documented in `AGENTS.md`
- [x] Uses centralized exclusion list
- [x] Tests for each rule
- [x] Smoke test: fires on any repo with `.sh` files
- [x] `devbox run -- just quality` passes

## Out of Scope

- **shellcheck / shfmt integration** — project-lint does static text checks, not running external linters. The `just lint` target should run shellcheck/shfmt separately.
- **Zsh / Fish scripts** — `.zsh` / `.fish` not covered. Future enhancement.

## Dependencies

- **Centralized exclusion list** — must not scan `node_modules/`, `target/`, `.devbox/gen/` (generated scripts).
