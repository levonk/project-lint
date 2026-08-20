# PreToolUse Hook Should Fire Before Permission Deny

**Date**: 2026-08-09
**Status**: Open
**Component**: Devin CLI hooks / permissions interaction

## Current Limitation

`permissions.deny` rules in `~/.config/devin/config.json` (or `.devin/config.json`) are evaluated **before** `PreToolUse` hooks. A denied tool call is blocked immediately and the hook never runs, so the hook cannot rewrite, log, or redirect the command. Observed: `Exec(npx)` in global `deny` blocks `npx skills find dns` with "Permission denied for this tool" — the project-lint `pnpm-enforcer` hook (installed at `.devin/hooks.v1.json`) never receives the event.

## Desired Improvement

One of:

1. **PreToolUse fires before the permission decision.** Hooks run on every tool call; a hook may rewrite the input (e.g. `npx` → `pnpm dlx`) and the rewritten command is then evaluated against `deny`/`ask`/`allow`. This lets a redirect hook rescue a command that would otherwise be denied for its *form* rather than its *intent*.
2. **Pass the denial verdict into the hook payload.** If deny must stay first, include a `permission_decision: "deny"` field (and the matched rule) in the `PreToolUse` stdin payload so the hook can log, notify, or surface a rewrite suggestion to the agent/user even though execution is blocked.

Option 1 enables transparent auto-redirect. Option 2 enables observability and user-facing suggestions without changing the security boundary.

## Workflow Example

```jsonc
// ~/.config/devin/config.json
{ "permissions": { "deny": ["Exec(npx)"] } }

// .devin/hooks.v1.json — project-lint pnpm-enforcer
{ "PreToolUse": [{ "matcher": "exec", "hooks": [{ "type": "command",
  "command": "project-lint hook --source claude" }] }] }
```

Agent runs `npx skills find dns`.

- **Today:** deny fires → "Permission denied" → hook skipped. No rewrite, no log, no chance to recover.
- **Desired (option 1):** PreToolUse hook runs → returns `hookSpecificOutput.updatedInput.command = "pnpm dlx skills find dns"` → permission check runs on the rewritten command → `pnpm dlx` is not in deny → executes.
- **Desired (option 2):** deny fires → hook still runs with `permission_decision: "deny"` in payload → hook logs the attempted `npx` use and emits a user-visible suggestion ("use `pnpm dlx skills find dns`") → command stays blocked.

## Impact / Context

- **Security boundary preserved either way.** Option 1 re-runs the permission check on the rewritten input, so a deny on `Exec(pnpm)` would still block. Option 2 doesn't execute anything new.
- **Unblocks hook-based redirect patterns.** project-lint's `pnpm-enforcer` rule (rewriting `npm`→`pnpm`, `npx`→`pnpm dlx`, `yarn`→`pnpm`, `bun`→`pnpm`) is currently dead on arrival for any user with a global `Exec(npx)` deny — a common hardening rule. Same applies to any `rtk`-wrapper or proxy-command hook design.
- **Affects all `PreToolUse` rewrite hooks**, not just project-lint. Any tool that transparently rewrites commands (wrappers, sandbox routers, audit shims) is defeated by a deny rule on the *pre-rewrite* form.
- **Discoverability is poor.** The docs describe deny precedence over ask/allow ([permissions.mdx](file:///usr/local/Caskroom/devin-cli/3000.2.17/share/devin/docs/reference/permissions.mdx)) and PreToolUse firing "before a tool executes" ([lifecycle-hooks.mdx](file:///usr/local/Caskroom/devin-cli/3000.2.17/share/devin/docs/extensibility/hooks/lifecycle-hooks.mdx)), but never state that deny short-circuits the hook. Users assume a rewrite hook will get a chance.
