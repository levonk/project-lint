# Worktree Isolation Enforcement

**Date**: 2026-08-31
**Status**: Partially shipped (PRs #7, #8 merged); enhancements #5-#9 open
**Component**: Hook engine / install-hook / git hooks
**PRs**: [#7](https://github.com/levonk/project-lint/pull/7), [#8](https://github.com/levonk/project-lint/pull/8)

## Problem

Work on protected branches (main, master, trunk, develop) should happen
inside a linked `git worktree add` worktree, not in the main checkout. The
main worktree must stay clean so it can be used as a stable reference, a
fast-forward target, and a recovery baseline. Before this feature,
project-lint had three gaps:

1. **Pre-commit hook** only checked include-catalog freshness; it did not
   enforce worktree isolation, despite the Developer Guide stating that
   commits to `main` outside a worktree should be blocked.
2. **PreToolUse hook** blocked `run_subagent` dispatch but not direct edits.
   A user worked around the subagent block by editing files directly
   instead of fixing the root cause (lack of worktree isolation).
3. **Stop hook** detected a dirty `main` but only fired once per session
   due to a loop guard. After stashing changes, recovery left the user
   unable to trigger the guard again.

Additionally, `install-hook --install` installed only Git hooks, not
Claude Code hooks, so the Claude `PreToolUse`/`Stop` integration was
missing entirely.

## Scope

### File types covered

- No file-type scanning — this is a hook-engine rule, not a scanner.
- Acts on `PreToolUse`, `PostToolUse`, `Stop`, and `SubagentStop` events.
- Path-scoped via `protected_paths` globs (default `["src/**"]`).

### Rules

- [x] `worktree-isolation-enforcer` (PreToolUse) — blocks direct edits
  (`Edit`/`Write`/`MultiEdit`/`NotebookEdit`) and subagent dispatch
  (`Task`/`run_subagent`) on a protected branch in the main worktree.
  Write-tool blocking is scoped by `protected_paths`; subagent dispatch is
  never path-scoped. **Severity: error. Auto-fixable: no.**
- [x] `worktree-isolation-enforcer` (PostToolUse) — re-runs the
  `protected_paths` + branch check after a write lands, catching writes
  that slipped through (e.g. hook bypassed with `--no-verify`, or a tool
  the matcher missed). **Severity: error. Auto-fixable: no.**
- [x] `worktree-isolation-enforcer` (Stop) — blocks stopping with a dirty
  working tree on a protected branch in the main worktree. Fires on every
  Stop event with no once-per-session suppression. **Severity: error.
  Auto-fixable: no.**
- [x] `worktree-isolation-enforcer` (SubagentStop) — same dirty-tree guard
  as Stop, but fires as soon as a subagent returns rather than waiting for
  the top-level Stop. **Severity: error. Auto-fixable: no.**
- [x] Git pre-commit gate — generated `pre-commit` script blocks commits
  to protected branches outside a linked worktree. **Severity: error
  (exit 1).**
- [x] Git pre-push gate — generated `pre-push` script blocks pushes to
  protected branches outside a linked worktree. **Severity: error
  (exit 1).**

### Configuration

TOML table: `.config/project-lint/rules/active/worktree-isolation.toml`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `protected_branches` | `Vec<String>` | `["main", "master", "trunk", "develop"]` | Branches where work is forbidden outside a linked worktree |
| `protected_paths` | `Vec<String>` | `["src/**"]` | Path globs scoping which write-tool events are blocked. Empty defaults to `["src/**"]`. Subagent dispatch is never path-scoped. |
| `triggers` | `Vec<String>` | `["pre_tool_use", "post_tool_use", "stop", "subagent_stop"]` | Hook events the rule evaluates |

### Installation

| Agent | What gets installed |
|-------|-------------------|
| `git-hooks` | `.git/hooks/pre-commit` (worktree gate + lint checks), `.git/hooks/pre-push` (worktree gate + lint checks) |
| `claude` | `.claude/hook.sh` + `.claude/settings.json` (registers PreToolUse + Stop hooks, non-destructive merge) |
| `all` | Both git hooks and Claude Code hooks in one command |

## Acceptance Criteria

### Shipped (PR #7 + #8)

- [x] Direct edits to `src/**` on `main` in the main worktree are blocked
  (PreToolUse returns Deny).
- [x] Direct edits to `docs/` or `README.md` on `main` in the main
  worktree are allowed (not in `protected_paths`).
- [x] Subagent dispatch on `main` in the main worktree is blocked
  regardless of path.
- [x] Edits on a feature branch are allowed.
- [x] Edits inside a linked worktree on `main` are allowed.
- [x] Stop with a dirty tree on `main` in the main worktree is blocked.
- [x] Stop with a clean tree on `main` is allowed.
- [x] A second Stop event with a still-dirty tree is blocked again (no
  once-per-session suppression).
- [x] SubagentStop with a dirty tree fires the guard immediately.
- [x] PostToolUse catches a write that slipped through to `src/` on main.
- [x] `protected_branches` is configurable via TOML (not hardcoded).
- [x] Pre-commit script blocks commits to protected branches outside a
  linked worktree.
- [x] Pre-push script blocks pushes to protected branches outside a
  linked worktree.
- [x] `install-hook --agent claude` creates `.claude/settings.json`
  (non-destructive merge with existing settings).
- [x] `install-hook --agent all` installs both git and Claude hooks.
- [x] MultiEdit/NotebookEdit file paths are resolved from `tool_input`
  (the Claude mapper doesn't populate `context.file_path` for these).
- [x] Git subprocesses scrub inherited `GIT_*` env vars so
  hook-inherited state (e.g. `GIT_INDEX_FILE`) doesn't corrupt worktree
  creation in tests.
- [x] `cargo test --workspace` passes (17 worktree tests + 3 install-hook
  tests).
- [x] `devbox run scripts/run-quality-checks.sh` passes.

### Open (enhancements #5-#9)

- [ ] **#5 `project-lint worktree` subcommand** — a `worktree start`
  command that reads `protected_branches` + naming convention from config,
  creates the linked worktree with a consistent name, and prints the path
  for the agent to cd into. Turns the error message from "go do X" into
  "run `project-lint worktree start`".
- [ ] **#6 Auto-install on `init`** — `project-lint init --install-hooks`
  (or a prompt) sets up git + Claude hooks in one shot, so worktree
  protection is active from the first commit.
- [ ] **#7 Worktree status in `lint` output** — `project-lint lint`
  prints a one-line status ("worktree isolation active, on protected
  branch 'main' in main worktree — 3 protected paths") so the protection
  is visible during normal lint runs, not only when a violation fires.
- [ ] **#8 Per-branch `protected_paths` scoping** — `protected_paths`
  becomes a map or list of `{branches, paths}` so a team can protect
  `src/**` on `main` but `src/**` + `docs/**` on `release/*`.
- [ ] **#9 Telemetry on worktree gate hits** — the hook logger records a
  counter for "worktree isolation blocked N edits, M subagent dispatches,
  K dirty-stops this session" so teams can see whether the guard catches
  real violations or just gets in the way.

## Out of Scope

- **Worktree creation automation** beyond the proposed `worktree start`
  subcommand (#5). project-lint will not manage worktree lifecycle
  (prune, remove, list).
- **Remote branch protection.** This is a local hook, not a server-side
  gate. Branch protection rules on GitHub/GitLab are orthogonal.
- **Non-Git VCS.** Only Git worktrees are supported.
- **Detached HEAD.** The rule no-ops on detached HEAD (no branch name to
  check against `protected_branches`).

## Implementation Notes

### Worktree detection

Linked worktrees are detected by comparing normalized
`git rev-parse --git-dir` vs `git rev-parse --git-common-dir` paths. In
the main worktree they resolve equivalently; in a linked worktree the
per-worktree Git directory differs from the shared common directory.

### Env scrubbing

Git subprocesses use `env_clear()` + a minimal `clean_env()` (PATH, HOME,
USER, TMPDIR, LANG, LC_ALL) to avoid hook-inherited `GIT_*` vars
corrupting worktree operations. This was needed because the pre-commit
hook sets `GIT_INDEX_FILE`, which caused `git worktree add` in tests to
fail with "index file open failed: Not a directory".

### Claude settings merge

`merge_claude_settings` in `src/commands/install_hook.rs` reads existing
`.claude/settings.json`, preserves all top-level keys and existing hook
entries, adds `PreToolUse` (matcher `Edit|Write|MultiEdit|NotebookEdit|Task`)
and `Stop` entries pointing at `.claude/hook.sh`, detects duplicate
commands to avoid re-adding, and only overwrites malformed/non-object
JSON when `--force` is supplied.
