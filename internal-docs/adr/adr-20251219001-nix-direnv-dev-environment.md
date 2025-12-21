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

<!-- vim: set ft=markdown: -->
