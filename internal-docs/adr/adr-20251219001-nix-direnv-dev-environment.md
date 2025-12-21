---
modeline: "vim: set ft=markdown:"
title: "ADR: Dev Environment via direnv + Nix flake"
adr-id: 20251219001
slug: 20251219001-nix-direnv-dev-environment
url: /internal-docs/adr/adr-20251219001-nix-direnv-dev-environment.md
synopsis: Use direnv and a Nix flake dev shell to provide required tools (for example universal-ctags) instead of system installs.
author: https://github.com/levonk
date-created: 2025-12-19
date-updated: 2025-12-19
version: 1.0.0
status: "accepted"
aliases: []
tags: ["doc/architecture/adr", "developer-experience", "nix", "direnv", "tooling"]
supersedes: []
superseded-by: []
related-to: []
---

## Context

Missing developer tools create friction and lead to inconsistent local setups. A recent example is `universal-ctags`, which is needed for indexing workflows.

## Decision

This repository will standardize on:

- `direnv` to automatically load a per-project environment.
- `flake.nix` to define a reproducible dev shell that provides required tools.

When a tool is missing, the fix is to add it to `flake.nix` (not to install it globally).

## Consequences

- **Positive**
  - Reproducible tooling across machines.
  - Lower setup time for new contributors.
  - Fewer "works on my machine" problems.

- **Negative**
  - Requires `nix` and `direnv` to be installed once per machine.

## Rollout

- Add `.envrc` and `flake.nix` to the repository.
- Contributors run `direnv allow` once per clone.

### Baseline Makefile targets

To keep setup repeatable, ship a minimal `Makefile` alongside `.envrc` and `flake.nix` with these targets:

- **bootstrap**: install Nix (if missing), install `direnv`, and run the flake to initialize the dev shell environment. (Non-destructive; idempotent.)
- **clean**: remove temporary build artifacts and Nix/direnv caches created by this repo’s workflow (do not remove the global Nix store).
- **build**: run the project’s primary build using the flake dev shell.
- **run**: start the main application/service in the flake dev shell.
- **deploy**: invoke the project’s deployment flow (document environment assumptions; keep non-prod by default).
- **lint**: run linters/formatters/tests wired into the flake dev shell.
- **help/usage**: print target list and short descriptions.

Notes:
- All targets execute under the flake-provided dev shell (no global tool assumptions).
- `bootstrap` is the only target allowed to check/install `nix` and `direnv`; all other targets assume they’re present and `direnv` is allowed.

<!-- vim: set ft=markdown: -->
