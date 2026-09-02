# Nix Stack Rules

Nix stack rules validate Nix-based development environment files: `flake.nix`,
`flake.lock`, `devbox.json`, `devbox.lock`, `shell.nix`, `default.nix`, and
`.envrc`. These scanners perform static text/JSON analysis — they do not run
`nix flake check` or evaluate Nix expressions.

## Overview

The nix stack scanners help identify:

- Missing or incomplete flake inputs and lockfiles
- Floating (unpinned) Nix package references
- Devbox schema violations and missing lockfiles
- Improper shell.nix structure (missing mkShell, floating nixpkgs)
- Hardcoded secrets and absolute paths in `.envrc` files
- Missing direnv/devbox integration directives

## Scanners

### nix_flake

Validates `flake.nix` and `flake.lock` files. Parses `flake.nix` with regex
(Nix is not trivially serde-parseable) and `flake.lock` as JSON.

**Check name**: `nix_flake`

#### flake.nix rules

| Rule | Severity | Description |
|------|----------|-------------|
| `flake-inputs-have-urls` | error | Every input must have a `url` field |
| `flake-inputs-pinned` | warning | Inputs should pin to a specific ref/rev |
| `flake-nixpkgs-not-floating` | info | `nixpkgs` should not use `nixpkgs-unstable` (when `require_stable_nixpkgs = true`) |
| `flake-outputs-function` | error | `outputs` must be a function |
| `flake-has-description` | info | Flake should have a `description` field |
| `flake-no-flake-false` | warning | `flake = false` in inputs should be intentional |

#### flake.lock rules

| Rule | Severity | Description |
|------|----------|-------------|
| `flake-lock-present` | error | `flake.lock` must exist when `flake.nix` exists |
| `flake-lock-fresh` | warning | All inputs in `flake.nix` should have lock entries |
| `flake-lock-nar-hash-present` | error | Every node must have `narHash` or `narinfo` |

#### Configuration

```toml
[scanner_config.nix_flake]
require_stable_nixpkgs = false
check_lock_freshness = true
```

### devbox_json

Validates `devbox.json` as JSON (not regex).

**Check name**: `devbox_json`

#### Schema rules

| Rule | Severity | Description |
|------|----------|-------------|
| `devbox-name-present` | info | Should have a `"name"` field |
| `devbox-packages-is-object` | error | `"packages"` must be an object, not an array |
| `devbox-schema-present` | info | Should have a `"$schema"` field |

#### Package pinning rules

| Rule | Severity | Description |
|------|----------|-------------|
| `devbox-no-floating-nixpkgs` | warning | `devbox.lock` must be present and committed |
| `devbox-lock-present` | error | `devbox.lock` must exist when `devbox.json` exists |
| `devbox-github-packages-pinned` | warning | GitHub packages should pin to a rev/tag |

#### Content rules

| Rule | Severity | Description |
|------|----------|-------------|
| `devbox-init-hook-not-empty` | info | `shell.init_hook` should not be an empty array |
| `devbox-scripts-use-just` | warning | `scripts` should delegate to `just` targets |
| `devbox-no-npx-bunx-yarn` | error | No `npx`, `bunx`, or `yarn` in scripts/init_hook |

#### Configuration

```toml
[scanner_config.devbox_json]
require_schema = true
require_lock = true
require_scripts_use_just = true
forbidden_commands = ["npx", "bunx", "yarn"]
```

### nix_shell

Validates `shell.nix` and `default.nix` files using text/regex analysis.

**Check name**: `nix_shell`

| Rule | Severity | Description |
|------|----------|-------------|
| `shell-nix-mkshell` | warning | `shell.nix` should use `pkgs.mkShell` |
| `shell-nix-buildinputs` | warning | `mkShell` should have `buildInputs` or `packages` |
| `shell-nix-no-floating-nixpkgs` | warning | Should not use `import <nixpkgs> {}` |
| `default-nix-not-shell` | info | `default.nix` should not be a shell definition |

#### Configuration

```toml
[scanner_config.nix_shell]
require_mkshell = true
forbid_floating_nixpkgs = true
```

### envrc_content

Validates `.envrc` files for direnv-based dev environments.

**Check name**: `envrc_content`

| Rule | Severity | Description |
|------|----------|-------------|
| `envrc-no-hardcoded-secrets` | error | No `export FOO=literal` patterns |
| `envrc-uses-devbox` | warning | Should use `use devbox` or `use flake` |
| `envrc-no-direnv-allow-rc` | info | Should not contain `direnv allow` |
| `envrc-watch-file-devbox` | warning | Should `watch_file devbox.json` when using devbox |
| `envrc-no-absolute-paths` | warning | No hardcoded absolute paths |

#### Configuration

```toml
[scanner_config.envrc_content]
require_devbox = true
require_watch_file = true
secret_patterns = ["^export\\s+\\w+=['\"]?[^\\s$({][^\\s]*"]
```

## Examples

### Valid flake.nix

```nix
{
  description = "My project flake";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils/main";
  };
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system}; in
      { devShells.default = pkgs.mkShell { buildInputs = [ pkgs.go ]; }; });
}
```

### Valid devbox.json

```json
{
  "name": "myproject",
  "$schema": "https://raw.githubusercontent.com/jetify-com/devbox/main/schema.json",
  "packages": { "go": "" },
  "shell": { "init_hook": ["just bootstrap_impl"] },
  "scripts": { "build": "just build_impl" }
}
```

### Valid .envrc

```bash
use devbox
watch_file devbox.json
```

## Out of Scope

- **Nix evaluation** — project-lint does not run `nix flake check` or evaluate
  Nix expressions. It does static text/JSON analysis.
- **NixOS module validation** — `module.nix` / `configuration.nix` for NixOS
  systems are not covered.
- **devbox plugin validation** — devbox plugins (`.devbox/`) are generated
  artifacts, not validated.
- **Flake-compat** — `flake-compat.nix` shim files are not validated.
