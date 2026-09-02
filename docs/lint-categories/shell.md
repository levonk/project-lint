# Shell Script Rules

Shell rules enforce best practices for `*.sh` / `*.bash` files: portable
shebangs, strict mode, guarded PATH additions, `exec` for final long-lived
commands, dirty-git gates for destructive scripts, and forbidden
package-manager commands.

## Configuration
```toml
[scanner_config.shell_script]
require_shebang = true
require_strict_mode = true
forbid_hardcoded_home = true
forbidden_commands = ["npx", "bunx", "yarn dlx"]
require_devbox_run = false  # set true in devbox projects
```

The scanner is gated by the `shell_script` check name. Disable it via:
```toml
[rules]
disabled_checks = ["shell_script"]
```

## Rules
- `sh-shebang` (warning) — scripts must start with `#!/usr/bin/env bash`
- `sh-strict-mode` (warning) — `set -euo pipefail` after the shebang
- `sh-exec-final-command` (info) — final long-lived command uses `exec`
- `sh-path-addition-guard` (warning) — PATH additions guarded against duplicates
- `sh-git-cleanliness-gate` (warning) — destructive scripts check dirty git state
- `sh-dry-run-first` (warning) — destructive scripts support a dry-run path
- `sh-bounded-timeout` (info) — long-running operations wrapped in `timeout`
- `sh-no-hardcoded-home` (warning) — no `/Users/<user>/`, `/home/<user>/`, `C:\Users\`
- `sh-no-npx-bunx-yarn` (error) — no `npx`, `bunx`, `yarn dlx` (use `pnpm dlx`)
- `sh-uses-devbox-run` (warning) — build tools invoked via `devbox run --` in devbox projects

## Examples
✅ Good:
```bash
#!/usr/bin/env bash
set -euo pipefail

case ":$PATH:" in *":$HOME/bin:"*) ;; *) export PATH="$HOME/bin:$PATH";; esac
exec node server.js
```

❌ Bad:
```bash
#!/bin/sh
npx prettier --write .
cd /Users/micro/projects
rm -rf dist
```

## Out of Scope
- **shellcheck / shfmt** — run separately via `just lint`; project-lint does
  static text checks only.
- **Zsh / Fish scripts** (`.zsh`, `.fish`) — future enhancement.
