# PRD: Container Stack (Compose Lint + Dockerfile Enhancements)

**Date**: 2026-09-01
**Status**: accepted
**Scope**: New `compose_lint` scanner for `docker-compose*.yml` /
`compose*.yml` files, plus enhancements to the existing
`dockerfile_lint` scanner to cover additional container hardening rules
from the `container-hardening.toml` modular rule slice and the
`secure-docker` workflow.

## Problem

The existing `dockerfile_lint` scanner covers 3 rules (pinned digests,
no COPY ., non-root USER). But:

1. **No compose file scanner** — 28 `docker-compose.yml` files across
   repos, none validated. Compose files have their own security concerns
   (privileged mode, no resource limits, no healthchecks, no
   `security_opt`, binding `0.0.0.0`).
2. **Dockerfile gaps** — missing HEALTHCHECK, `:latest` tag detection,
   no `--no-cache` for `apk`/`apt-get`, multi-stage build detection,
   `.dockerignore` presence.

## File Types Covered

| File type | Count (levonk+lrepo52) | Scanner |
|-----------|----------------------|---------|
| `Dockerfile` / `Dockerfile.*` / `*.dockerfile` | ~37 | dockerfile_lint (existing) |
| `docker-compose.yml` / `docker-compose.yaml` | ~28 | compose_lint (new) |
| `compose.yml` / `compose.yaml` / `compose.*.yml` | ~9 | compose_lint (new) |
| `.dockerignore` | ~5 | dockerfile_lint (new check) |

## Rules

### compose_lint (check name: `compose_lint`) — NEW SCANNER

#### Image rules
- [ ] `compose-pinned-images` — Service images must be pinned by digest (`image: foo@sha256:...`), not just by tag. **Severity: error.** Auto-fixable: no.
- [ ] `compose-no-latest-tag` — Services must not use `:latest` tag. **Severity: error.** Auto-fixable: no.
- [ ] `compose-no-floating-tag` — Services should not use floating major tags (`:20`, `:3`) without digest pinning. **Severity: warning.** Auto-fixable: no.

#### Security rules
- [ ] `compose-no-privileged` — Services must not run in privileged mode (`privileged: true`). **Severity: error.** Auto-fixable: no.
- [ ] `compose-security-opt` — Services should have `security_opt: ["no-new-privileges:true"]`. **Severity: warning.** Auto-fixable: no.
- [ ] `compose-no-docker-sock-mount` — Services must not mount `/var/run/docker.sock` unless exempted by `com.dockerproxy.role` label. **Severity: error.** Auto-fixable: no.
- [ ] `compose-no-root-user` — Services should specify `user: <non-root>` or `user: "1000:1000"`. **Severity: warning.** Auto-fixable: no.
- [ ] `compose-no-host-network` — Services should not use `network_mode: host`. **Severity: warning.** Auto-fixable: no.
- [ ] `compose-no-host-pid` — Services should not use `pid: host`. **Severity: warning.** Auto-fixable: no.
- [ ] `compose-cap-drop` — Services should have `cap_drop: ["ALL"]` and only add back specific capabilities. **Severity: warning.** Auto-fixable: no.
- [ ] `compose-readonly-filesystem` — Long-running services should have `read_only: true` with explicit `tmpfs` for write paths. **Severity: info.** Auto-fixable: no.

#### Health rules
- [ ] `compose-healthcheck` — Long-running services should define `healthcheck`. **Severity: warning.** Auto-fixable: no.
- [ ] `compose-restart-policy` — Services should define `restart: unless-stopped` or `restart: always` for production. **Severity: info.** Auto-fixable: no.

#### Resource rules
- [ ] `compose-resource-limits` — Services should define `deploy.resources.limits` (memory, cpus). **Severity: warning.** Auto-fixable: no.
- [ ] `compose-no-bind-0.0.0.0` — Port bindings should not bind to `0.0.0.0` without gateway annotation. Use `127.0.0.1:PORT:PORT` for local-only. **Severity: warning.** Auto-fixable: no.

#### Update rules
- [ ] `compose-watchtower-labels` — Long-running services should have watchtower/wud update labels (`com.centurylinklabs.watchtower.enable=true` or `wud.tag.include=regex`). **Severity: info.** Auto-fixable: no. **Note**: Only checked when `ops_mode = true` in config.

### dockerfile_lint enhancements (check name: `dockerfile_lint`) — EXISTING SCANNER

#### New rules (in addition to existing 3)
- [ ] `dockerfile-no-latest-tag` — `FROM` must not use `:latest` tag. **Severity: error.** Auto-fixable: no.
- [ ] `dockerfile-healthcheck` — Dockerfile should define `HEALTHCHECK` instruction. **Severity: warning.** Auto-fixable: no.
- [ ] `dockerfile-no-new-privileges` — Not applicable to Dockerfile directly (handled at compose/runtime level), but Dockerfile should not disable security features. **Severity: N/A.** Skip.
- [ ] `dockerfile-apk-no-cache` — `apk add` commands must use `--no-cache` flag. **Severity: warning.** Auto-fixable: yes (add `--no-cache`).
- [ ] `dockerfile-apt-get-no-install-recommends` — `apt-get install` must use `--no-install-recommends`. **Severity: warning.** Auto-fixable: yes.
- [ ] `dockerfile-apt-get-clean` — `apt-get install` must be followed by `rm -rf /var/lib/apt/lists/*`. **Severity: warning.** Auto-fixable: no.
- [ ] `dockerfile-dockerignore-present` — Project with a Dockerfile should have `.dockerignore`. **Severity: warning.** Auto-fixable: no.
- [ ] `dockerfile-multi-stage` — Dockerfiles with `RUN` install commands should use multi-stage builds to reduce image size. **Severity: info.** Auto-fixable: no.
- [ ] `dockerfile-distroless-scratch-exempt` — `FROM scratch` and `gcr.io/distroless/static:nonroot` are exempt from digest pinning. **Severity: N/A.** Auto-fixable: no.

## Implementation

### ComposeLintScanner (new file: `project-lint-core/src/scanners/compose_lint.rs`)

```rust
pub struct ComposeLintScanner {
    require_pinned_digests: bool,
    require_healthcheck: bool,
    require_resource_limits: bool,
    require_no_new_privileges: bool,
    forbid_privileged: bool,
    forbid_docker_sock: bool,
    ops_mode: bool,  // enables watchtower/update checks
    exempt_proxy_labels: Vec<String>,  // e.g. ["com.dockerproxy.role"]
}
```

The scanner walks the project for compose files, parses each as YAML
(using `serde_yaml`), and checks each service against the rules.

**YAML parsing**: Use `serde_yaml` to parse compose files into a
`ComposeFile` struct with `services: HashMap<String, Service>`. This
is more robust than regex on YAML. Add `serde_yaml` to
`project-lint-core/Cargo.toml` if not already present.

### DockerfileLintScanner (enhance existing)

Add new checks to the existing `scan_dockerfile()` method:
- Track `HEALTHCHECK` instruction presence
- Check `apk add` for `--no-cache`
- Check `apt-get install` for `--no-install-recommends` and cleanup
- Check for `:latest` in `FROM` (separate from digest pinning)
- Exempt `scratch` and `distroless` from digest pinning

Add a new `scan_project()` check for `.dockerignore` presence.

## Configuration

```toml
[scanner_config.compose_lint]
require_pinned_digests = true
require_healthcheck = true
require_resource_limits = false  # warn-only by default
require_no_new_privileges = true
forbid_privileged = true
forbid_docker_sock = true
ops_mode = false  # set true to enable watchtower/update checks
exempt_proxy_labels = ["com.dockerproxy.role"]

[scanner_config.dockerfile_security]
# Existing fields
require_pinned_digests = true
require_non_root_user = true
forbid_copy_dot = true
# New fields
require_healthcheck = true
require_apk_no_cache = true
require_apt_no_install_recommends = true
require_dockerignore = true
exempt_from_digest_pinning = ["scratch", "gcr.io/distroless/static:nonroot"]
```

## Acceptance Criteria

- [ ] `ComposeLintScanner` exists with `scan()` returning `Vec<ScannerIssue>`
- [ ] `ComposeLintScanner` is registered in `mod.rs`
- [ ] `ComposeLintScanner` is wired into `lint.rs::run` with `is_check_enabled("compose_lint")` gate
- [ ] `ComposeLintScanner` has config struct in `config.rs`
- [ ] `ComposeLintScanner` uses `serde_yaml` for parsing (not regex)
- [ ] `ComposeLintScanner` uses centralized exclusion list
- [ ] `DockerfileLintScanner` has new rules implemented
- [ ] `DockerfileLintScanner` existing tests still pass
- [ ] New tests for each new rule (positive + negative + edge case)
- [ ] Smoke test: scanner is silent on repos without Dockerfiles/compose files
- [ ] Smoke test: scanner fires on `agentmemory` (has Dockerfiles + docker-compose.yml)
- [ ] `AGENTS.md` updated with both scanners
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **Kubernetes manifests** — `*.yaml` K8s files are not compose files. A future `k8s_lint` scanner would handle these.
- **Container image scanning** — scanning the actual image layers (trivy, grype) is out of scope. project-lint validates the Dockerfile/compose definitions, not the built images.
- **BuildKit / docker buildx** — build-specific features are not validated.
- **Docker swarm** — `docker stack deploy` files have different semantics. The compose scanner focuses on `docker compose` v2.

## Dependencies

- **Centralized exclusion list** — scanner must not scan `node_modules/`, `target/`, etc.
- **`serde_yaml` crate** — needed for compose file parsing. Check if already in `Cargo.toml`; if not, add it.
