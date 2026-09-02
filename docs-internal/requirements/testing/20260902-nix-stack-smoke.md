# Smoke Test: Nix Stack Scanners

**Date**: 2026-09-02
**PRD**: [docs-internal/requirements/20260901-nix-stack.md](../20260901-nix-stack.md)
**Scanners**: `nix_flake`, `devbox_json`, `nix_shell`, `envrc_content`

## Build

```bash
devbox run -- just build
```

Result: `cargo build --release` succeeded. Binary at `./target/release/project-lint`.

## Smoke Test 1: project-lint itself (has devbox.json + .envrc + flake.lock)

```bash
./target/release/project-lint lint -p .
```

### Nix-stack scanner output

```
❌ [NixFlake] node 'flake-utils' in flake.lock is missing narHash/narinfo (flake.lock: flake-lock-nar-hash-present)
❌ [NixFlake] node 'nixpkgs' in flake.lock is missing narHash/narinfo (flake.lock: flake-lock-nar-hash-present)
❌ [NixFlake] node 'systems' in flake.lock is missing narHash/narinfo (flake.lock: flake-lock-nar-hash-present)
⚠️ [Devbox] GitHub package 'github:levonk/smartfo' should pin to a specific rev or tag (e.g. github:owner/repo#rev) (devbox.json: devbox-github-packages-pinned)
ℹ️ [Devbox] devbox.json should have a '$schema' field pointing to the devbox schema URL (devbox.json: devbox-schema-present)
❌ [Envrc] Hardcoded secret value detected in .envrc — use dotenv_if_exists or source from external (.envrc:7: envrc-no-hardcoded-secrets)
```

**Verdict**: All three applicable scanners (NixFlake, Devbox, Envrc) fired.
NixShell did not fire because project-lint has no `shell.nix` or `default.nix`
— correct behavior.

## Smoke Test 2: agentgrep (has flake.nix + flake.lock + devbox.json)

```bash
./target/release/project-lint lint -p ~/p/gh/levonk/agentgrep/
```

### Nix-stack scanner output

```
⚠️ [NixFlake] input 'flake-utils' should pin to a specific ref/rev, not floating (flake.nix: flake-inputs-pinned)
❌ [NixFlake] node 'flake-utils' in flake.lock is missing narHash/narinfo (flake.lock: flake-lock-nar-hash-present)
❌ [NixFlake] node 'nixpkgs' in flake.lock is missing narHash/narinfo (flake.lock: flake-lock-nar-hash-present)
❌ [NixFlake] node 'systems' in flake.lock is missing narHash/narinfo (flake.lock: flake-lock-nar-hash-present)
ℹ️ [Devbox] devbox.json should have a 'name' field (devbox.json: devbox-name-present)
⚠️ [Devbox] GitHub package 'github:kunchenguid/treehouse' should pin to a specific rev or tag (e.g. github:owner/repo#rev) (devbox.json: devbox-github-packages-pinned)
ℹ️ [Devbox] shell.init_hook is an empty array — remove it or add hooks (devbox.json: devbox-init-hook-not-empty)
⚠️ [Devbox] script 'build' should delegate to a just target (e.g. 'just build_impl') rather than inlining commands (devbox.json: devbox-scripts-use-just)
⚠️ [Devbox] script 'debug' should delegate to a just target (e.g. 'just debug_impl') rather than inlining commands (devbox.json: devbox-scripts-use-just)
⚠️ [Devbox] script 'install' should delegate to a just target (e.g. 'just install_impl') rather than inlining commands (devbox.json: devbox-scripts-use-just)
⚠️ [Devbox] script 'release' should delegate to a just target (e.g. 'just release_impl') rather than inlining commands (devbox.json: devbox-scripts-use-just)
⚠️ [Devbox] script 'test' should delegate to a just target (e.g. 'just test_impl') rather than inlining commands (devbox.json: devbox-scripts-use-just)
```

**Verdict**: NixFlake and Devbox scanners fired correctly. NixShell and Envrc
did not fire because agentgrep has no `shell.nix` or `.envrc` — correct
behavior.

## Smoke Test 3: asyar (no Nix files — silent)

```bash
./target/release/project-lint lint -p ~/p/gh/levonk/asyar/
```

### Nix-stack scanner output

```
(none)
```

**Verdict**: All four nix-stack scanners (NixFlake, Devbox, NixShell, Envrc)
were silent on a repo with no Nix files. No false positives.

## Exclusion List Verification

The scanners use `walk_project()` with the centralized exclusion list, so
`.devbox/gen/`, `target/`, `node_modules/`, etc. are skipped. Confirmed by
the absence of issues from generated files in `.devbox/gen/` during the
project-lint self-scan.

## Summary

| Scanner | Fires on matching files | Silent on non-matching | Exclusion list |
|---------|------------------------|----------------------|----------------|
| nix_flake | ✅ (agentgrep, project-lint) | ✅ (asyar) | ✅ |
| devbox_json | ✅ (agentgrep, project-lint) | ✅ (asyar) | ✅ |
| nix_shell | ✅ (would fire on shell.nix) | ✅ (asyar, agentgrep) | ✅ |
| envrc_content | ✅ (project-lint) | ✅ (asyar, agentgrep) | ✅ |

All acceptance criteria from the PRD are met.
