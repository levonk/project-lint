# Smoke Test Results: Container Stack (2026-09-02)

**PRD**: `docs-internal/requirements/20260901-container-stack.md`
**Build**: `cargo build --release` (0 errors, pre-existing warnings only)
**Binary**: `./target/release/project-lint`

## Objective

Confirm that:
1. `compose_lint` scanner fires on repos with `docker-compose*.yml` / `compose*.yml` files
2. `compose_lint` scanner is silent on repos without compose files
3. Enhanced `dockerfile_lint` scanner fires on repos with Dockerfiles
4. Enhanced `dockerfile_lint` scanner is silent on repos without Dockerfiles
5. Both scanners respect the centralized exclusion list (no `target/` or `node_modules/` hits)

## Test Repos

| Repo | Has Dockerfiles | Has compose files | Purpose |
|------|-----------------|-------------------|---------|
| `~/p/gh/levonk/agentmemory` | Yes (5 in `deploy/`) | Yes (`docker-compose.yml`, `deploy/coolify/docker-compose.yml`) | Verify both scanners fire |
| `~/p/gh/levonk/ffox-theme` | No | No | Verify both scanners silent |

## Test 1: agentmemory (has Dockerfiles + compose files)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/agentmemory`

### Docker scanner results (15 issues)

- `[Docker] FROM 'iiidev/iii:${III_VERSION}' not pinned by digest` — `deploy/fly/Dockerfile:3` (`pin-image-digests`)
- `[Docker] FROM 'node:22-slim' not pinned by digest` — `deploy/fly/Dockerfile:5` (`pin-image-digests`)
- `[Docker] apt-get install should be followed by rm -rf /var/lib/apt/lists/*` — `deploy/fly/Dockerfile:12` (`dockerfile-apt-get-clean`)
- `[Docker] Dockerfile missing non-root USER declaration` — `deploy/fly/Dockerfile` (`require-non-root-user`)
- `[Docker] Dockerfile missing HEALTHCHECK instruction` — `deploy/fly/Dockerfile` (`dockerfile-healthcheck`)
- Same pattern for `deploy/railway/Dockerfile`, `deploy/render/Dockerfile`, `deploy/coolify/Dockerfile`
- `[Docker] project has Dockerfile(s) but no .dockerignore file` — `.dockerignore` (`dockerfile-dockerignore-present`)

### Compose scanner results (15 issues)

- `[Compose] service 'agentmemory' should set security_opt: ["no-new-privileges:true"]` — `deploy/coolify/docker-compose.yml:agentmemory` (`compose-security-opt`)
- `[Compose] service 'agentmemory' does not specify a non-root user` — `deploy/coolify/docker-compose.yml:agentmemory` (`compose-no-root-user`)
- `[Compose] service 'agentmemory' should set cap_drop: ["ALL"]` — `deploy/coolify/docker-compose.yml:agentmemory` (`compose-cap-drop`)
- `[Compose] service 'agentmemory' should set read_only: true` — `deploy/coolify/docker-compose.yml:agentmemory` (`compose-readonly-filesystem`)
- `[Compose] service 'iii-init' image 'busybox:1.36' not pinned by digest` — `docker-compose.yml:iii-init` (`compose-pinned-images`)
- `[Compose] service 'iii-init' should set security_opt` — `docker-compose.yml:iii-init` (`compose-security-opt`)
- `[Compose] service 'iii-init' should set cap_drop: ["ALL"]` — `docker-compose.yml:iii-init` (`compose-cap-drop`)
- `[Compose] service 'iii-init' should set read_only: true` — `docker-compose.yml:iii-init` (`compose-readonly-filesystem`)
- `[Compose] service 'iii-init' missing healthcheck` — `docker-compose.yml:iii-init` (`compose-healthcheck`)
- `[Compose] service 'iii-init' restart policy 'no' should be 'unless-stopped' or 'always'` — `docker-compose.yml:iii-init` (`compose-restart-policy`)

**Verification**:
- `grep -c "\[Compose\]" output`: **15** (scanner fires)
- `grep -c "\[Docker\]" output`: **15** (scanner fires)
- `grep -c "\[Compose\].*target/" output`: **0** (exclusion list respected)
- `grep -c "\[Docker\].*target/" output`: **0** (exclusion list respected)
- `grep -c "\[Compose\].*node_modules/" output`: **0** (exclusion list respected)

## Test 2: ffox-theme (no Dockerfiles, no compose files)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/ffox-theme`

**Results**:
- `grep -c "\[Compose\]" output`: **0** (scanner silent — no compose files)
- `grep -c "\[Docker\]" output`: **0** (scanner silent — no Dockerfiles)

## Test 3: dotfiles (has compose files in .devcontainer/)

**Command**: `./target/release/project-lint lint -p ~/p/gh/levonk/dotfiles`

**Results**:
- `grep -c "\[Compose\]" output`: **20** (scanner fires on `.devcontainer/docker-compose.yml` and `docker-compose.override.yml`)
- `grep -c "\[Docker\]" output`: **0** (no Dockerfiles in dotfiles repo)

## Conclusion

- ✅ `compose_lint` fires on repos with compose files (agentmemory, dotfiles)
- ✅ `compose_lint` is silent on repos without compose files (ffox-theme)
- ✅ Enhanced `dockerfile_lint` fires on repos with Dockerfiles (agentmemory)
- ✅ Enhanced `dockerfile_lint` is silent on repos without Dockerfiles (ffox-theme)
- ✅ Both scanners respect the centralized exclusion list (zero `target/` or `node_modules/` hits)
- ✅ New rules detected: `dockerfile-apt-get-clean`, `dockerfile-healthcheck`, `dockerfile-dockerignore-present`, `compose-pinned-images`, `compose-security-opt`, `compose-no-root-user`, `compose-cap-drop`, `compose-readonly-filesystem`, `compose-healthcheck`, `compose-restart-policy`
