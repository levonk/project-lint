# Rule Source Index: Skills & Workflows → PRD Cross-Reference

**Date**: 2026-09-01
**Status**: living document
**Purpose**: Maps every enforceable rule discovered in skills and
workflows to the PRD that covers it. This ensures no rule from the
knowledge base is lost during implementation. When a PRD is
implemented, the corresponding rule here should be checked off.

## Sources Scanned

1. **Skills**: `~/p/gh/levonk/skills-src/build/{current,private,prototype}/skills/` — all SKILL.md files + referenced knowledge bundles
2. **Knowledge bundles**: `~/p/gh/levonk/skills-src/build/current/knowledge/` — 25 bundles covering dev environment, container, Rust, TS monorepo, CI/CD, security, data engineering, etc.
3. **Workflows**: `~/p/gh/levonk/project-lint/.agents/workflows/` + `~/p/gh/levonk/skills-src/build/current/workflows/` — workflow definitions with process and file-type rules

## Cross-Reference: Rules → PRDs

### SKILL.md / INSTRUCTIONS.md rules → PRD: Wire Dead Scanners (markdown_frontmatter)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| frontmatter-required-fields (name, description, version) | skills/AGENTS.md | wire-dead-scanners | ✅ covered by `md-frontmatter-title` (partial — needs name/description/version specific check) |
| frontmatter-date-fields (created, knowledge-basis, last-used) | skills/AGENTS.md + nixify | wire-dead-scanners | ⚠️ gap — PRD doesn't cover date fields for SKILL.md specifically |
| refresh-before-last-used | nixify/SKILL.md | wire-dead-scanners | ⚠️ gap — PRD doesn't cover refresh.sh presence |
| includeForCache-in-header | skills/AGENTS.md | wire-dead-scanners | ⚠️ gap — PRD doesn't cover includeForCache |
| no-inlined-script-code | skills/AGENTS.md | wire-dead-scanners | ⚠️ gap — PRD doesn't cover script inlining |
| no-hardcoded-paths | skills/AGENTS.md | wire-dead-scanners | ✅ covered by general `md-frontmatter-*` |
| body-length-limit (~500 lines) | skills/AGENTS.md | wire-dead-scanners | ⚠️ gap — existing `skill_markdown` scanner has 80-line limit, not 500 |
| required-scripts-refresh | skills/AGENTS.md + project-lint AGENTS.md | wire-dead-scanners | ✅ covered by existing `skill_markdown` scanner |

**Action**: The existing `skill_markdown` scanner already covers some SKILL.md rules (body limit, refresh.sh, frontmatter fields). The `markdown_frontmatter` scanner covers general markdown frontmatter. Need to reconcile — SKILL.md-specific rules (name/description/version, date fields, includeForCache) should be in `skill_markdown`, not `markdown_frontmatter`.

### AGENTS.md rules → PRD: Wire Dead Scanners (markdown_frontmatter) + new AGENTS.md scanner

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| binding-contract-exists | skills/AGENTS.md | N/A | ⚠️ gap — no PRD for AGENTS.md validation |
| no-current-to-private-leak | private/AGENTS.md | N/A | ⚠️ gap — skills-src specific |
| no-build-artifacts-committed | skills-src/AGENTS.md | N/A | ✅ covered by centralized exclusion list (prevents scanning, not committing) |
| no-secrets-committed | skills-src/AGENTS.md | N/A | ✅ covered by existing `security` scanner |
| no-advertising-attribution | skills-src/AGENTS.md + global AGENTS.md | N/A | ⚠️ gap — no PRD for commit message validation |
| use-git-mv | skills-src/AGENTS.md | N/A | ⚠️ gap — git operation validation, not file content |

**Action**: Need a new PRD for AGENTS.md validation (binding contract structure, Usage Protocol presence, JIT Index format).

### devbox.json rules → PRD: Nix Stack (devbox_json)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| project-uses-devbox | dev-environment-practices | nix-stack | ✅ covered by existing `dev_environment` scanner |
| packages-runtimes-not-libraries | dev-environment-practices/multi-language | nix-stack | ⚠️ gap — PRD doesn't check for `python*Packages.*` in devbox.json |
| init-hook-frozen-install | dev-environment-practices/multi-language | nix-stack | ⚠️ gap — PRD doesn't check for `--frozen-lockfile` / `--frozen` |
| scripts-point-to-just | dev-environment-practices/standard-ux | nix-stack | ✅ covered by `devbox-scripts-use-just` |

**Action**: Add `devbox-no-language-libraries`, `devbox-init-hook-frozen-install` rules to nix-stack PRD.

### .envrc rules → PRD: Nix Stack (envrc_content)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| use-direnv | dev-environment-practices | nix-stack | ✅ covered by existing `dev_environment` (presence) |
| use-devbox-shellenv | dev-environment-practices/direnv-rustup | nix-stack | ✅ covered by `envrc-uses-devbox` |
| watch-devbox-files | dev-environment-practices/direnv-rustup | nix-stack | ✅ covered by `envrc-watch-file-devbox` (partial — needs devbox.lock + rust-toolchain.toml + Cargo.toml) |
| scope-rustup-cargo-home | dev-environment-practices/direnv-rustup | nix-stack | ⚠️ gap — PRD doesn't check for RUSTUP_HOME/CARGO_HOME scoping |
| project-local-path-precedence | dev-environment-practices/multi-language | nix-stack | ⚠️ gap — PRD doesn't check PATH_add ordering |
| no-synchronous-cargo-rustup | dev-environment-practices/direnv-rustup | nix-stack | ⚠️ gap — PRD doesn't check for blocking cargo/rustup calls |

**Action**: Add `envrc-rustup-cargo-home`, `envrc-path-precedence`, `envrc-no-blocking-cargo` rules to nix-stack PRD.

### justfile rules → PRD: Build/CI Stack (justfile_content)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| prefer-justfile-over-makefile | dev-environment-practices/just-over-makefiles | build-ci-stack | ✅ covered by existing `dev_environment` (Makefile forbidden) |
| _devbox-helper-required | dev-environment-practices/standard-ux | build-ci-stack | ⚠️ gap — PRD doesn't check for `_devbox` helper recipe |
| impl-recipe-naming | dev-environment-practices/standard-ux | build-ci-stack | ⚠️ gap — PRD doesn't check for `*_impl` suffix pattern |
| standard-recipe-set | dev-environment-practices/standard-ux | build-ci-stack | ✅ covered by `justfile-quality-target` etc. (partial — needs `dev`, `bootstrap`, `doctor`) |

**Action**: Add `justfile-devbox-helper`, `justfile-impl-naming`, `justfile-standard-recipes` rules to build-ci-stack PRD.

### Makefile rules → PRD: Build/CI Stack (makefile_content)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| mandatory-targets (clean, archive, all, help, check, format, test, lint, coverage, version, status, watch, env, docs, sync) | build-system-essentials | build-ci-stack | ⚠️ gap — PRD only checks for forbidden Makefile, not target content |
| .PHONY-all-commands | build-system-essentials | build-ci-stack | ⚠️ gap |
| default-goal-help | build-system-essentials | build-ci-stack | ⚠️ gap |

**Action**: Add Makefile target validation rules to build-ci-stack PRD.

### pnpm-workspace.yaml / package.json rules → PRD: Monorepo Stack + Wire Dead Scanners

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| use-pnpm-only (only-allow pnpm) | ts-monorepo-best-practices | monorepo-stack | ✅ covered by existing `pnpm_lockfile` scanner (partial — needs preinstall check) |
| external-deps-in-catalog | ts-monorepo-best-practices | monorepo-stack | ✅ covered by `pnpm-workspace-catalog` |
| catalogMode-strict | ts-monorepo-best-practices | monorepo-stack | ✅ covered by `pnpm-workspace-catalog` |
| no-asterisk-version | ts-monorepo-best-practices | monorepo-stack | ⚠️ gap — PRD doesn't check for `"*"` in package.json deps |
| workspace-protocol | ts-monorepo-best-practices | monorepo-stack | ⚠️ gap — PRD doesn't check for `workspace:*` protocol |
| no-npx-bunx-yarn-dlx | ts-monorepo-best-practices + skills-src/AGENTS.md | wire-dead-scanners + monorepo-stack | ✅ covered by `package-json-no-npm-scripts` (partial — needs npx/bunx/yarn) |
| container-exception-bunx | ts-monorepo-best-practices | container-stack | ⚠️ gap — container PRD doesn't mention bunx exception |
| packageManager-pnpm | ts-monorepo-best-practices | monorepo-stack | ✅ covered by `node-modules-package-manager-field` |

**Action**: Add `package-json-no-asterisk-deps`, `package-json-workspace-protocol` to monorepo-stack PRD. Add bunx container exception note to container-stack PRD.

### nx.json rules → PRD: Monorepo Stack (nx_config)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| cli-packageManager-pnpm | ts-monorepo-best-practices | monorepo-stack | ⚠️ gap — PRD doesn't check `cli.packageManager` |
| namedInputs-correct (sharedGlobals includes lock files) | ts-monorepo-best-practices | monorepo-stack | ✅ covered by `nx-named-inputs` (partial — needs sharedGlobals check) |
| target-defaults (build dependsOn ^build, cache: true) | ts-monorepo-best-practices | monorepo-stack | ✅ covered by `nx-target-defaults` |

**Action**: Add `nx-cli-package-manager` rule to monorepo-stack PRD.

### Dockerfile / compose rules → PRD: Container Stack

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| pin-base-image-digest | container-best-practices | container-stack | ✅ covered by existing `pin-image-digests` |
| set-node-env-production | container-best-practices/nodejs | container-stack | ⚠️ gap — PRD doesn't check for NODE_ENV=production |
| non-root-user | container-best-practices | container-stack | ✅ covered by existing `require-non-root-user` |
| use-init-for-signals (dumb-init) | container-best-practices/nodejs | container-stack | ⚠️ gap — PRD doesn't check for dumb-init ENTRYPOINT |
| npm-ci-omit-dev | container-best-practices/nodejs | container-stack | ⚠️ gap — PRD doesn't check for `npm ci` vs `npm install` |
| buildkit-secret-mounts | container-best-practices/buildkit-secrets | container-stack | ⚠️ gap — PRD doesn't check for --mount=type=secret |
| package-cache-cleanup | container-best-practices | container-stack | ✅ covered by `dockerfile-apk-no-cache` + `dockerfile-apt-get-clean` |
| HEALTHCHECK-required | container-best-practices | container-stack | ✅ covered by `dockerfile-healthcheck` |
| multi-stage-build | container-best-practices | container-stack | ✅ covered by `dockerfile-multi-stage` |
| .dockerignore-required-entries | container-best-practices/nodejs | container-stack | ⚠️ gap — PRD checks .dockerignore presence but not content |
| non-root-read-only (compose) | container-best-practices/runtime-hardening | container-stack | ✅ covered by `compose-readonly-filesystem` + `compose-no-root-user` |
| no-docker-socket-mount | container-best-practices/runtime-hardening | container-stack | ✅ covered by `compose-no-docker-sock-mount` |
| no-unauthenticated-dockerd-tcp | container-best-practices/runtime-hardening | container-stack | ⚠️ gap — PRD doesn't check for dockerd TCP exposure |
| capability-drop | container-best-practices/runtime-hardening | container-stack | ✅ covered by `compose-cap-drop` |
| security-opt-no-new-privileges | container-best-practices/runtime-hardening | container-stack | ✅ covered by `compose-security-opt` |
| resource-limits | container-best-practices/runtime-hardening | container-stack | ✅ covered by `compose-resource-limits` |

**Action**: Add `dockerfile-node-env-production`, `dockerfile-dumb-init`, `dockerfile-npm-ci`, `dockerfile-buildkit-secrets`, `dockerignore-content`, `compose-no-dockerd-tcp` rules to container-stack PRD.

### flake.nix rules → PRD: Nix Stack (nix_flake)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| packages-default-output | nixify/INSTRUCTIONS.md | nix-stack | ⚠️ gap — PRD checks outputs function but not packages.default specifically |
| upstream-references-for-forks | nixify/INSTRUCTIONS.md | nix-stack | ⚠️ gap — PRD doesn't check upstream references |
| platform-scope-restricted | nixify/INSTRUCTIONS.md | nix-stack | ⚠️ gap — PRD doesn't check meta.platforms |
| version-from-latest-release | nixify/INSTRUCTIONS.md | nix-stack | ⚠️ gap — PRD doesn't check version source |
| package-manager-builder-match | nixify/INSTRUCTIONS.md | nix-stack | ⚠️ gap — PRD doesn't check builder type matches lockfile |
| nixpkgs-output-when-superset | nixify/INSTRUCTIONS.md | nix-stack | ⚠️ gap |

**Action**: Add nixify-specific flake rules to nix-stack PRD. These are important for the `nixify` workflow.

### Cargo.toml / Rust rules → PRD: Language Configs (rust_conventions)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| rust-toolchain-toml | dev-environment-practices/direnv-rustup | language-configs | ⚠️ gap — PRD doesn't check for rust-toolchain.toml presence |
| .rustfmt.toml canonical-config | rust-dev-practices/rustfmt-clippy | language-configs | ⚠️ gap — PRD doesn't validate .rustfmt.toml content |
| clippy.toml threshold-config | rust-dev-practices/rustfmt-clippy | language-configs | ⚠️ gap — PRD doesn't validate clippy.toml content |
| security-deps (secrecy, zeroize) | rust-dev-practices/security-auditing | language-configs | ⚠️ gap — PRD doesn't check for security crates |
| cargo-audit-in-validate | rust-dev-practices/security-auditing | language-configs | ⚠️ gap — PRD doesn't check justfile for cargo audit |

**Action**: Add `rust-toolchain-toml-present`, `rustfmt-config-canonical`, `clippy-config-thresholds`, `cargo-security-crates`, `justfile-cargo-audit` rules to language-configs PRD.

### Shell / Bash rules → PRD: new Shell Script scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| shebang-and-strict-mode | dev-environment-practices/shell-scripting | N/A | ⚠️ gap — no PRD for shell script validation |
| use-exec-for-final-command | dev-environment-practices/shell-scripting | N/A | ⚠️ gap |
| path-addition-guard | dev-environment-practices/shell-scripting | N/A | ⚠️ gap |
| git-cleanliness-gate | dev-environment-practices/shell-scripting | N/A | ⚠️ gap |
| dry-run-first | dev-environment-practices/shell-scripting | N/A | ⚠️ gap |
| shellcheck-shfmt-clean | dev-environment-practices/shell-scripting | N/A | ⚠️ gap |
| bounded-timeout | dev-environment-practices/shell-scripting | N/A | ⚠️ gap |
| no-hardcoded-home | nixify/INSTRUCTIONS.md | N/A | ⚠️ gap |

**Action**: Need a new PRD for shell script validation (`shell_script` scanner). 561 `.sh` files found in scan.

### GitHub Actions rules → PRD: Build/CI Stack (github_workflow)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| workflow-security-zizmor | cicd-testing-practices/pre-commit-ci-parity | build-ci-stack | ⚠️ gap — PRD doesn't require zizmor in CI |
| workflow-security-actionlint | cicd-testing-practices/pre-commit-ci-parity | build-ci-stack | ⚠️ gap — PRD doesn't require actionlint in CI |
| no-mutable-action-refs | cicd-testing-practices/pre-commit-ci-parity | build-ci-stack | ✅ covered by `workflow-pinned-actions` |
| no-write-all-permissions | cicd-testing-practices/pre-commit-ci-parity | build-ci-stack | ✅ covered by `workflow-permissions-minimal` (partial) |
| no-pull-request-target-injection | cicd-testing-practices/pre-commit-ci-parity | build-ci-stack | ✅ covered by `workflow-no-pull-request-target` |

**Action**: Add `workflow-ci-runs-zizmor`, `workflow-ci-runs-actionlint`, `workflow-no-write-all` rules to build-ci-stack PRD.

### TypeScript / JavaScript rules → PRD: Wire Dead Scanners (config_validation) + new TS scanner

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| explicit-module-extensions (.mts/.cts not .ts/.js) | frontend-stack-practices | wire-dead-scanners | ⚠️ gap — config_validation doesn't check file extensions |
| jsx-only-in-tsx | frontend-stack-practices | wire-dead-scanners | ⚠️ gap |

**Action**: Add `ts-no-ambiguous-extensions`, `ts-jsx-only-in-tsx` rules. These could go in the existing `typescript` scanner or a new `typescript_extensions` scanner.

### Security / cross-cutting rules → PRD: existing security scanner + new PRDs

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-hardcoded-credentials (AWS, Stripe, Google, GitHub, JWT, private keys) | devsecops-codeguard | N/A | ✅ covered by existing `security` scanner |
| no-hardcoded-secrets | skills-src/AGENTS.md | N/A | ✅ covered by existing `security` scanner |
| no-absolute-home-paths | nixify + skills-src/AGENTS.md | N/A | ⚠️ gap — no scanner checks for /Users/ or /home/ paths in committed files |
| no-npx-bunx-yarn-dlx | skills-src/AGENTS.md + ts-monorepo | multiple | ⚠️ partially covered — needs cross-file check (not just package.json) |
| use-devbox-run | skills-src/AGENTS.md | build-ci-stack | ✅ covered by `justfile-uses-devbox-run` + `workflow-uses-devbox` (partial — needs all files) |

**Action**: Add `no-absolute-home-paths` rule to a cross-cutting scanner (could be in `security` or a new `path_hygiene` scanner). Add `no-npx-bunx-yarn-dlx` as a cross-file check.

### Template / Python script rules → PRD: new PRD needed

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| .tmpl triple-brace-delimiters | skills/AGENTS.md | N/A | ⚠️ gap — no PRD for .tmpl validation |
| .tmpl no-bare-relative-cross-ref | skills/AGENTS.md | N/A | ⚠️ gap |
| .py PEP-723-inline-metadata | skills/AGENTS.md | N/A | ⚠️ gap — no PRD for Python script PEP 723 |
| .py run-via-uv | skills/AGENTS.md | N/A | ⚠️ gap |

**Action**: Need PRDs for `.tmpl` validation (skills-src specific) and Python script PEP 723 validation.

### Submodule / project structure rules → PRD: existing scanners + new PRD

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| read-only-submodule (vendor/) | skills-src/AGENTS.md | N/A | ✅ covered by existing `submodule_integrity` scanner (partial) |
| submodule-pin-rebuild | skills-src/AGENTS.md | N/A | ⚠️ gap — no scanner checks pin/HEAD consistency |
| packages-category-platform | project-lint/AGENTS.md | N/A | ✅ covered by existing `package_organization` scanner |
| knowledge-bundle-layout (index.md, overview.md, log.md) | skills-src/AGENTS.md | N/A | ⚠️ gap — no scanner validates knowledge bundle structure |
| skill-directory-layout | skills/AGENTS.md | N/A | ⚠️ gap — no scanner validates skill directory structure |

**Action**: Need PRD for knowledge bundle and skill directory structure validation (skills-src specific).

## Workflow-Derived Rules (from .agents/workflows/ scan)

### Project-lint scanner meta-rules → PRD: lint-upsert.md (self-enforcing)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| scanner-four-touchpoints | lint-upsert.md | lint-upsert | ✅ enforced by workflow |
| scanner-module-structure | lint-upsert.md | lint-upsert | ✅ enforced by workflow |
| scanner-colocated-tests | lint-upsert.md + project-lint-execute.md | lint-upsert | ✅ enforced by workflow |
| prd-required-fields | lint-upsert.md | lint-upsert | ✅ enforced by workflow |
| apply-fixes-impl | project-lint-execute.md | lint-upsert | ✅ enforced by workflow |
| no-absolute-paths-committed | project-lint-execute.md | agents-md-validation | ✅ covered by `no-absolute-home-paths` |
| quality-gate-before-commit | project-lint-execute.md | lint-upsert | ✅ enforced by workflow |
| conventional-commits | project-lint-execute.md | N/A (commit-level) | ⚠️ gap — need commit message scanner |
| no-literal-newline (PR body) | project-lint-execute.md | N/A (PR-level) | ⚠️ gap — PR body validation, not file-level |
| exclusion-list-respected | lint-upsert.md | centralized-exclusion-list | ✅ covered |

### Docker standards workflow rules → PRD: Container Stack

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| dockerfile-location (docker/ subdir) | docker-standards.md | container-stack | ⚠️ gap — PRD doesn't check Dockerfile location |
| docker-assets-directory | docker-standards.md | container-stack | ⚠️ gap |
| runtime-templates-directory | docker-standards.md | container-stack | ⚠️ gap |
| docker-compose-present at service root | docker-standards.md | container-stack | ⚠️ gap — PRD checks compose content, not presence at root |
| compose-no-version-key | docker-standards.md | container-stack | ⚠️ gap — PRD doesn't check for deprecated `version:` key |
| compose-yaml-leading-document-start (---) | docker-standards.md | container-stack | ⚠️ gap |
| compose-container-name-pattern (localnet-app-) | docker-standards.md | container-stack | ⚠️ gap — project-specific naming convention |
| docker-no-privileged | docker-standards.md | container-stack | ✅ covered by `compose-no-privileged` |
| docker-socket-restrictions | docker-standards.md | container-stack | ✅ covered by `compose-no-docker-sock-mount` |
| no-secrets-in-layers | docker-standards.md | container-stack | ⚠️ gap — PRD doesn't check for ENV SECRET= patterns |
| healthcheck-instruction | docker-standards.md | container-stack | ✅ covered by `dockerfile-healthcheck` |
| multi-stage-builds | docker-standards.md | container-stack | ✅ covered by `dockerfile-multi-stage` |
| port-exposure-localhost (127.0.0.1 binding) | docker-standards.md | container-stack | ✅ covered by `compose-no-bind-0.0.0.0` |
| env-port-format ({CATEGORY}_{SERVICE}_...) | docker-standards.md | container-stack | ⚠️ gap — env var naming convention |
| standard-env-variables (PUID, PGID, TZ, UMASK) | docker-standards.md | container-stack | ⚠️ gap |
| docker-README-present | docker-standards.md | container-stack | ⚠️ gap — README presence check for Docker projects |
| docker-test-directory | docker-standards.md | container-stack | ⚠️ gap |
| healthcheck-scripts (healthcheck/ dir) | docker-standards.md | container-stack | ⚠️ gap |
| security-scan-make-target | docker-standards.md | container-stack | ⚠️ gap — Makefile target for security scanning |
| docker-make-targets (full list) | docker-standards.md | build-ci-stack | ⚠️ gap — comprehensive Makefile target list for Docker projects |
| security-scan-ci | docker-standards.md | build-ci-stack | ⚠️ gap — CI must run Trivy/Dockle/Hadolint |

### Nix standards workflow rules → PRD: Nix Stack

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| use-nix-flakes | nix-standards.md | nix-stack | ✅ covered by `flake-outputs-function` (partial) |
| no-legacy-channels | nix-standards.md | nix-stack | ✅ covered by `shell-nix-no-floating-nixpkgs` (partial — needs <nixpkgs> check) |
| commit-lockfile | nix-standards.md | nix-stack | ✅ covered by `flake-lock-present` |
| pin-inputs | nix-standards.md | nix-stack | ✅ covered by `flake-inputs-pinned` |
| no-secrets-in-nix | nix-standards.md | nix-stack | ⚠️ gap — PRD doesn't check for secrets in .nix files |

### Kubernetes rules → PRD: new K8s scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| default-deny-networkpolicy | k8s-standards.md | N/A | ⚠️ gap — no PRD for K8s manifest validation |
| resource-quota | k8s-standards.md | N/A | ⚠️ gap |
| container-limits | k8s-standards.md | N/A | ⚠️ gap |
| liveness-readiness-probes | k8s-standards.md | N/A | ⚠️ gap |

### Helm rules → PRD: new Helm scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| helm-lint-and-template | helm/USAGE-EXAMPLE.md | N/A | ⚠️ gap — no PRD for Helm chart validation |
| helm-values-files | helm/USAGE-EXAMPLE.md | N/A | ⚠️ gap |
| helm-validate-existing-service | helm/add-helm-to-deployable.md | N/A | ⚠️ gap |
| helm-hpa-conditional | helm/add-helm-to-deployable.md | N/A | ⚠️ gap |

### Pulumi workflow rules → PRD: IaC Stack (pulumi_lint)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| pull-request-preview | pulumi-essentials.md | iac-stack | ⚠️ gap — PRD checks Pulumi.yaml content, not CI preview requirement |
| least-privilege | pulumi-essentials.md | iac-stack | ⚠️ gap — PRD doesn't check IAM least-privilege |

### Python workflow rules → PRD: Language Configs (python_config) + new Python script scanner

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| pep723-header | python-best-practices.md | N/A | ⚠️ gap — need PEP 723 validation in scripts/*.py |
| declare-deps-in-header | python-best-practices.md | N/A | ⚠️ gap |
| no-inline-pip-install | python-best-practices.md | N/A | ⚠️ gap |
| modern-type-hints (list[str] not List[str]) | python-best-practices.md | N/A | ⚠️ gap — AST-level check |
| concrete-exceptions (no bare except) | python-best-practices.md | N/A | ⚠️ gap — AST-level check |
| parameterized-sql | python-best-practices.md | N/A | ⚠️ gap |
| no-poetry-forced-migration | python-best-practices.md | language-configs | ⚠️ gap — PRD doesn't check for forced migration |

### TypeScript / frontend workflow rules → PRD: Wire Dead Scanners + new TS scanner

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-any-types | roo-code-mode-rule.md | N/A | ⚠️ gap — AST-level check, not config validation |
| no-console-log-production | roo-code-mode-rule.md | N/A | ⚠️ gap |
| no-raw-stack-traces | roo-code-mode-rule.md | N/A | ⚠️ gap |
| parameterized-queries-orm | roo-code-mode-rule.md | N/A | ⚠️ gap |
| react-functional-hooks | roo-code-mode-rule.md | N/A | ⚠️ gap |
| type-safety-over-lint (no @ts-ignore) | dev-std10-cycle.md | N/A | ⚠️ gap |
| spacing-divisible-by-4-8 | ai-design-system.md | N/A | ⚠️ gap — design token check |

### Ansible workflow rules → PRD: IaC Stack (ansible_lint)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| variable-doc-table | ansible-document10-role.md | iac-stack | ⚠️ gap — PRD doesn't check variable documentation |
| explicit-activation-enum | ansible-document10-role.md | iac-stack | ⚠️ gap |
| galaxy-commit-lf | galaxy-commit.md | N/A | ⚠️ gap — commit-level, not file-level |
| no-state-restarted | ansible-document10-role.md | iac-stack | ⚠️ gap — PRD doesn't check for `state: restarted` |
| community-docker-modules | ansible-document10-role.md | iac-stack | ⚠️ gap |
| source-pull-not-build | ansible-document10-role.md | iac-stack | ⚠️ gap |

### Nx workflow rules → PRD: Monorepo Stack (nx_config)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-source-fix-nx-only | nxify.md | N/A | ⚠️ gap — PR-level check, not file-level |
| required-public-repo-values | nxify.md | monorepo-stack | ⚠️ gap |
| devbox-json (test, lint, graph, affected:* scripts) | nxify.md | nix-stack | ⚠️ gap — PRD checks scripts→just, not Nx-specific scripts |
| pnpm-nx-commands (pnpm exec nx, not npx) | nxify.md | monorepo-stack | ⚠️ gap — PRD doesn't check for `pnpm exec nx` usage |

### Git / commit workflow rules → PRD: new commit message scanner

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-ai-co-authored | skills-src-git.md + dotfiles-git.md | agents-md-validation | ✅ covered by `no-ai-attribution` (partial — needs commit-level check) |
| lf-only-commit-messages | galaxy-commit.md + chore-ai20-vcs.md | N/A | ⚠️ gap — commit-level check |
| conventional-commits-scopes | project-lint-execute.md | N/A | ⚠️ gap — commit-level check |
| skill-grm-provenance-only | skills-src-git.md | N/A | ⚠️ gap — skills-src specific |
| no-push-unless-asked | chore-ai20-vcs.md | N/A | ⚠️ behavior rule, not lintable |
| porcelain-status-before-clean | chore-ai20-vcs.md | N/A | ⚠️ behavior rule, not lintable |

### Dotfiles / rtk workflow rules → PRD: Shell Script Validation

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| rtk-shell-prefix | do-task-dotfiles.md | shell-script-validation | ⚠️ gap — PRD doesn't check for `rtk` prefix |
| no-chezmoi-apply | dotfiles-execute.md | N/A | ⚠️ gap — dotfiles specific |
| hook-tests | dotfiles-execute.md | N/A | ⚠️ gap — test presence check for hooks |

### Chezmoi rules → PRD: new Chezmoi scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-tmpl-in-chezmoitemplates | chezmoi-templating.md | N/A | ⚠️ gap — no PRD for chezmoi validation |
| escape-template-examples | chezmoi-templating.md | N/A | ⚠️ gap |
| idempotent-scripts | chezmoi-scripts.md | N/A | ⚠️ gap |
| no-direct-destination-changes | chezmoi-locations.md | N/A | ⚠️ gap — behavior rule |
| script-relative-paths | chezmoi-locations.md | N/A | ⚠️ gap |

### Nushell rules → PRD: new Nushell scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-hardcoded-paths (use path join) | nushell-guide.md | N/A | ⚠️ gap — no PRD for .nu files |
| test-file-naming (*test*.nu) | nutest-guide.md | N/A | ⚠️ gap |
| test-annotation (@test) | nutest-guide.md | N/A | ⚠️ gap |

### Infrahub / services.yml rules → PRD: new Infrahub scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| source-repo-required | infrahub-add-new-service.md | N/A | ⚠️ gap — no PRD for services.yml validation |
| top-level-services-key | infrahub-add-new-service.md | N/A | ⚠️ gap |
| client-override-only | infrahub-add-new-service.md | N/A | ⚠️ gap |
| required-service-fields | infrahub-add-new-service.md | N/A | ⚠️ gap |
| no-edit-generated-services | infrahub-add-new-service.md | N/A | ⚠️ gap |
| no-state-restarted (Ansible) | infrahub-add-new-service.md | iac-stack | ⚠️ gap — duplicate of ansible rule |
| no-docker-compose-raw | infrahub-add-new-service.md | iac-stack | ⚠️ gap |
| multi-stage-builds | infrahub-add-new-service.md | container-stack | ✅ covered by `dockerfile-multi-stage` |
| no-vault-direct-edit | infrahub-add-new-service.md | N/A | ⚠️ behavior rule |
| no-commit-secrets | infrahub-update-all.md | N/A | ✅ covered by existing `security` scanner |
| no-hardcoded-ips | infrahub-update-all.md | N/A | ⚠️ gap |
| no-docker-compose-remote | infrahub-update-all.md | N/A | ⚠️ behavior rule |
| image-age-2-days | infrahub-update-all.md | N/A | ⚠️ runtime check, not static |
| pre-post-update-git-tags | infrahub-update-all.md | N/A | ⚠️ behavior rule |
| no-docker-inspect-secrets | infrahub-update-all.md | N/A | ⚠️ behavior rule |
| secret-env-only-docker-run | infrahub-update-all.md | N/A | ⚠️ behavior rule |
| clean-repo-before-start | infrahub-update-all.md | N/A | ⚠️ behavior rule |
| no-skip-verify | infrahub-update-all.md | N/A | ⚠️ behavior rule |

### Task / story file rules → PRD: new task file scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| filename-format (tasks-[PRD]-[PHASE]-[ID]-[STORY].md) | tasks-from-prd.md | N/A | ⚠️ gap — no PRD for task file validation |
| index-status-column | tasks-from-prd.md | N/A | ⚠️ gap |
| dependencies-across-phases | tasks-from-prd.md | N/A | ⚠️ gap |
| commit-with-acceptance | tasks-processor.md | N/A | ⚠️ behavior rule |
| definition-of-done | tasks.md | N/A | ⚠️ gap |
| tkr-working-directory | task-tracking-ticketr.md | N/A | ⚠️ behavior rule |

### Cross-cutting workflow rules → PRD: AGENTS.md Validation + Path Hygiene

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-absolute-home-paths | decision-record.md + base-workflow-guidance.md | agents-md-validation | ✅ covered by `no-absolute-home-paths` |
| no-advertising-in-commits | skills-src-git.md + dotfiles-git.md | agents-md-validation | ✅ covered by `no-ai-attribution` |
| devbox-run-prefix | skills-src-execute.md + multiple do-task workflows | shell-script-validation + build-ci-stack | ✅ covered by `sh-uses-devbox-run` + `justfile-uses-devbox-run` + `workflow-uses-devbox` |
| pnpm-not-npx | levonk-base-boilerplate-execute.md + multiple | multiple | ✅ covered across multiple PRDs |
| justfile-quality-targets | project-lint-execute.md + lint-upsert.md | build-ci-stack | ✅ covered by `justfile-quality-target` + `justfile-quality-full-target` |
| git-mv | skills-src/AGENTS.md | N/A | ⚠️ behavior rule, not statically lintable |
| no-build-artifacts-committed | skills-src/AGENTS.md | centralized-exclusion-list | ✅ covered (prevents scanning, not committing) |
| no-secrets-committed | skills-src/AGENTS.md | N/A | ✅ covered by existing `security` scanner |
| read-root-agents | infrahub-git.md + project-lint-execute.md | agents-md-validation | ⚠️ behavior rule, not statically lintable |
| architecture-updated | lint-upsert.md | lint-upsert | ✅ enforced by workflow |

### 2ndbrain notes rules → PRD: new notes scanner (not yet created)

| Rule | Source | PRD | Status |
|------|--------|-----|--------|
| no-duplicate-notes | do-note-2ndbrain.md | N/A | ⚠️ gap — 2ndbrain specific |

## Updated Summary of Gaps Found

### New PRDs needed (not yet written)

1. **Shell script validation** — 561 `.sh` files, 8 rules from shell-scripting best practices ✅ DONE
2. **AGENTS.md validation** — binding contract structure, Usage Protocol, JIT Index format ✅ DONE
3. **Template (.tmpl) validation** — skills-src specific, triple-brace delimiters, no bare cross-refs
4. **Knowledge bundle structure** — index.md + overview.md + log.md in each bundle
5. **Skill directory structure** — SKILL.md + scripts/ + references/ layout
6. **Commit message validation** — no advertising/attribution, Conventional Commits format, LF-only
7. **Cross-cutting path hygiene** — no absolute home paths in any committed file ✅ DONE
8. **Python script PEP 723** — inline metadata headers in scripts/*.py, no pip install, modern type hints
9. **Kubernetes manifest validation** — default-deny NetworkPolicy, ResourceQuota, container limits, probes
10. **Helm chart validation** — Chart.yaml structure, values files, helm lint/template validation
11. **Chezmoi validation** — .chezmoitemplates naming, idempotent scripts, relative paths
12. **Nushell validation** — path join usage, test file naming, @test annotation
13. **Infrahub services.yml validation** — source_repo required, top-level services key, required fields
14. **Task file validation** — filename format, index status column, dependencies across phases
15. **TypeScript code rules** — no any types, no console.log in production, no @ts-ignore, parameterized queries
16. **2ndbrain notes validation** — no duplicate notes (lrepo52 specific)

### Rules to add to existing PRDs

- **container-stack**: `dockerfile-location`, `docker-assets-directory`, `runtime-templates-directory`, `compose-no-version-key`, `compose-yaml-leading-document-start`, `compose-container-name-pattern`, `no-secrets-in-layers`, `env-port-format`, `standard-env-variables`, `docker-README-present`, `docker-test-directory`, `healthcheck-scripts`, `dockerfile-node-env-production`, `dockerfile-dumb-init`, `dockerfile-npm-ci`, `dockerfile-buildkit-secrets`, `dockerignore-content`, `compose-no-dockerd-tcp`, bunx container exception
- **nix-stack**: `devbox-no-language-libraries`, `devbox-init-hook-frozen-install`, `envrc-rustup-cargo-home`, `envrc-path-precedence`, `envrc-no-blocking-cargo`, `nix-no-secrets`, nixify flake rules (packages.default, upstream refs, platform scope, builder match), `no-legacy-channels` (explicit <nixpkgs> check)
- **build-ci-stack**: `justfile-devbox-helper`, `justfile-impl-naming`, `justfile-standard-recipes`, Makefile target validation (full Docker target list), `workflow-ci-runs-zizmor`, `workflow-ci-runs-actionlint`, `workflow-no-write-all`, `security-scan-ci`
- **monorepo-stack**: `package-json-no-asterisk-deps`, `package-json-workspace-protocol`, `nx-cli-package-manager`, `nx-pnpm-exec-commands`, `nx-required-public-repo-values`
- **language-configs**: `rust-toolchain-toml-present`, `rustfmt-config-canonical`, `clippy-config-thresholds`, `cargo-security-crates`, `justfile-cargo-audit`, `python-no-poetry-forced-migration`
- **wire-dead-scanners**: SKILL.md-specific frontmatter rules (name/description/version, date fields), `ts-no-ambiguous-extensions`, `ts-jsx-only-in-tsx`
- **iac-stack**: `ansible-variable-doc-table`, `ansible-explicit-activation-enum`, `ansible-no-state-restarted`, `ansible-community-docker-modules`, `ansible-source-pull-not-build`, `pulumi-pr-preview-in-ci`, `pulumi-least-privilege`
- **shell-script-validation**: `sh-rtk-prefix` (dotfiles repos), `sh-no-chezmoi-apply`

<!-- vim: set ft=markdown -->
