# Master PRD Index: Project-Lint Scanner Expansion

**Date**: 2026-09-01
**Status**: living document
**Purpose**: Master index of all PRDs for the project-lint scanner
expansion. Tracks implementation status, dependencies, and priority
order. This is the entry point for the `lint-upsert.md` workflow —
pick a PRD from here, run the workflow, update the status.

## How to Use

1. Pick a PRD from the priority-ordered list below
2. Run the `lint-upsert.md` workflow against it
3. Update the status column when the 5-layer gate passes
4. Move to the next PRD

## PRD Inventory

### Foundation (must be done first — other PRDs depend on these)

| # | PRD | Check names | New scanners | Status | Depends on |
|---|-----|-------------|-------------|--------|------------|
| 1 | [centralized-exclusion-list](20260901-centralized-exclusion-list.md) | (utility) | 0 (shared util) | proposed | — |
| 2 | [wire-dead-scanners](20260901-wire-dead-scanners.md) | `config_validation`, `markdown_frontmatter`, `runtime_guards` | 0 (adapter wrappers for 3 existing) | proposed | #1 |

### High priority (high file count, high value, existing code to build on)

| # | PRD | Check names | New scanners | Status | Depends on |
|---|-----|-------------|-------------|--------|------------|
| 3 | [container-stack](20260901-container-stack.md) | `compose_lint` + `dockerfile_lint` (enhance) | 1 new + 1 enhanced | proposed | #1 |
| 4 | [nix-stack](20260901-nix-stack.md) | `nix_flake`, `devbox_json`, `nix_shell`, `envrc_content` | 4 new | proposed | #1 |
| 5 | [build-ci-stack](20260901-build-ci-stack.md) | `github_workflow`, `dependabot`, `justfile_content`, `makefile_content`, `process_compose` | 5 new | proposed | #1 |
| 6 | [monorepo-stack](20260901-monorepo-stack.md) | `nx_config`, `nx_project`, `pnpm_workspace` (enhance), `node_modules_integrity` | 3 new + 1 enhanced | proposed | #1 |
| 7 | [shell-script-validation](20260901-shell-script-validation.md) | `shell_script` | 1 new | proposed | #1 |
| 8 | [agents-md-validation](20260901-agents-md-validation.md) | `agents_md`, `path_hygiene` | 2 new | proposed | #1 |

### Medium priority (lower file count but important for completeness)

| # | PRD | Check names | New scanners | Status | Depends on |
|---|-----|-------------|-------------|--------|------------|
| 9 | [language-configs](20260901-language-configs.md) | `python_config`, `go_config`, `gradle_config`, `rust_conventions` (enhance) | 3 new + 1 enhanced | proposed | #1 |
| 10 | [iac-stack](20260901-iac-stack.md) | `terraform_lint`, `pulumi_lint`, `ansible_lint`, `jinja_template` | 4 new (forward-looking) | proposed | #1 |
| 11 | [data-api-stack](20260901-data-api-stack.md) | `sql_migration`, `protobuf_lint`, `prisma_schema` | 3 new | proposed | #1 |
| 12 | [binary-validation](20260901-binary-validation.md) | `binary_validation` | 1 new | proposed | #1 |

### Cross-reference

| # | Document | Purpose | Status |
|---|----------|---------|--------|
| 13 | [rule-source-index](20260901-rule-source-index.md) | Maps skills/workflows rules → PRDs | living |
| 14 | [worktree-isolation](20260831-worktree-isolation.md) | Worktree isolation enforcement (hook engine + git hooks + Claude install) | partially shipped (PRs #7, #8); enhancements #5-#9 open |

## Summary Statistics

- **Total PRDs**: 12 (excluding index)
- **Total new scanners**: 28
- **Total enhanced scanners**: 5
- **Total new rules**: ~150+
- **Foundation PRDs**: 2 (must be done first)
- **High priority PRDs**: 6
- **Medium priority PRDs**: 4

## File-Type Coverage Matrix (after all PRDs implemented)

| File type | Scanner | Check name | PRD # |
|-----------|---------|------------|-------|
| `Dockerfile` / `*.dockerfile` | dockerfile_lint | `dockerfile_lint` | #3 |
| `docker-compose*.yml` / `compose*.yml` | compose_lint | `compose_lint` | #3 |
| `.dockerignore` | dockerfile_lint | `dockerfile_lint` | #3 |
| `flake.nix` / `flake.lock` | nix_flake | `nix_flake` | #4 |
| `devbox.json` / `devbox.lock` | devbox_json | `devbox_json` | #4 |
| `shell.nix` / `default.nix` | nix_shell | `nix_shell` | #4 |
| `.envrc` | envrc_content | `envrc_content` | #4 |
| `.github/workflows/*.yml` | github_workflow | `github_workflow` | #5 |
| `.github/dependabot.yml` | dependabot | `dependabot` | #5 |
| `justfile` / `Justfile` | justfile_content | `justfile_content` | #5 |
| `Makefile` | makefile_content | `makefile_content` | #5 |
| `process-compose.yaml` | process_compose | `process_compose` | #5 |
| `nx.json` | nx_config | `nx_config` | #6 |
| `project.json` (Nx) | nx_project | `nx_project` | #6 |
| `pnpm-workspace.yaml` | pnpm_workspace | `pnpm_workspace` | #6 |
| `node_modules/` structure | node_modules_integrity | `node_modules_integrity` | #6 |
| `*.sh` / `*.bash` | shell_script | `shell_script` | #7 |
| `AGENTS.md` / `CLAUDE.md` | agents_md | `agents_md` | #8 |
| All text files (path hygiene) | path_hygiene | `path_hygiene` | #8 |
| `tsconfig.json` | config_validation | `config_validation` | #2 |
| `eslint.config.*` | config_validation | `config_validation` | #2 |
| `tailwind.config.*` | config_validation | `config_validation` | #2 |
| `package.json` | config_validation | `config_validation` | #2 |
| `*.md` (frontmatter) | markdown_frontmatter | `markdown_frontmatter` | #2 |
| `*.ts` / `.tsx` / `.js` / `.jsx` (browser guards) | runtime_guards | `runtime_guards` | #2 |
| `pyproject.toml` | python_config | `python_config` | #9 |
| `go.mod` / `go.sum` | go_config | `go_config` | #9 |
| `build.gradle` / `settings.gradle` | gradle_config | `gradle_config` | #9 |
| `Cargo.toml` (enhanced) | rust_conventions | `rust_conventions` | #9 |
| `*.tf` / `*.tfvars` | terraform_lint | `terraform_lint` | #10 |
| `Pulumi.yaml` | pulumi_lint | `pulumi_lint` | #10 |
| Ansible playbooks / `ansible.cfg` | ansible_lint | `ansible_lint` | #10 |
| `*.j2` / `*.jinja2` | jinja_template | `jinja_template` | #10 |
| `*.sql` (migrations) | sql_migration | `sql_migration` | #11 |
| `*.proto` | protobuf_lint | `protobuf_lint` | #11 |
| `*.prisma` | prisma_schema | `prisma_schema` | #11 |
| `*.png` / `*.jpg` / `*.gif` / `*.svg` / `*.pdf` / `*.mp4` / etc. | binary_validation | `binary_validation` | #12 |

## Backlog (identified gaps, PRDs not yet written)

These were identified in the rule-source-index but don't have PRDs yet:

### From skills scan

1. **Template (.tmpl) validation** — skills-src specific (triple-brace delimiters, no bare cross-refs)
2. **Knowledge bundle structure** — index.md + overview.md + log.md in each bundle
3. **Skill directory structure** — SKILL.md + scripts/ + references/ layout
4. **TypeScript extension enforcement** — .mts/.cts instead of .ts/.js
5. **Cross-file npx/bunx/yarn check** — check all files, not just package.json

### From workflow scan

6. **Commit message validation** — no advertising/attribution, Conventional Commits format, LF-only, no AI co-authored trailers
7. **Python script PEP 723** — inline metadata headers in scripts/*.py, no pip install, modern type hints, concrete exceptions, parameterized SQL
8. **Kubernetes manifest validation** — default-deny NetworkPolicy, ResourceQuota, container limits, liveness/readiness probes
9. **Helm chart validation** — Chart.yaml structure, values files, helm lint/template validation, HPA conditional
10. **Chezmoi validation** — .chezmoitemplates naming (no .tmpl), idempotent scripts, relative paths, no direct destination changes
11. **Nushell validation** — path join usage (no hardcoded `/`), test file naming (*test*.nu), @test annotation
12. **Infrahub services.yml validation** — source_repo required, top-level services key, required service fields, no hardcoded IPs
13. **Task file validation** — filename format (tasks-[PRD]-[PHASE]-[ID]-[STORY].md), index status column, dependencies across phases, definition of done
14. **TypeScript code rules** — no any types, no console.log in production, no @ts-ignore, parameterized queries, React functional hooks only
15. **2ndbrain notes validation** — no duplicate notes (lrepo52 specific)
16. **PR body validation** — no literal `\n`, must use `--body-file` (enforced by workflow, not a scanner)

## Updated Statistics (after workflow scan)

- **Total PRDs**: 12 (excluding index) + 11 backlog PRDs identified = 23 total
- **Total new scanners**: 28 + ~11 from backlog = ~39 total
- **Total enhanced scanners**: 5
- **Total new rules**: ~150 (skills) + ~100 (workflows) = ~250+
- **Foundation PRDs**: 2 (must be done first)
- **High priority PRDs**: 6
- **Medium priority PRDs**: 4
- **Backlog PRDs**: 11

<!-- vim: set ft=markdown -->
