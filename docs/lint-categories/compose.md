# Container / Compose Rules

Container rules detect insecure Dockerfile and docker-compose configurations,
enforcing image pinning, non-root execution, capability dropping, and resource
limits.

## Overview

Container rules help identify:
- Unpinned or floating image tags
- Privileged mode and excessive capabilities
- Missing healthchecks and restart policies
- Host network/PID namespace sharing
- Docker socket exposure
- Missing `.dockerignore` and multi-stage builds

## Configuration

### Compose Lint

Create `.config/project-lint/` config with:

```toml
[scanner_config.compose_lint]
require_pinned_digests = true
require_healthcheck = true
require_resource_limits = false
require_no_new_privileges = true
forbid_privileged = true
forbid_docker_sock = true
ops_mode = false
exempt_proxy_labels = ["com.dockerproxy.role"]
```

Enable the check in your profile or rules:

```toml
[rules]
enabled_checks = ["compose_lint"]
```

### Dockerfile Lint

```toml
[scanner_config.dockerfile_security]
require_pinned_digests = true
require_non_root_user = true
forbid_copy_dot = true
require_healthcheck = true
require_apk_no_cache = true
require_apt_no_install_recommends = true
require_dockerignore = true
exempt_from_digest_pinning = ["scratch", "gcr.io/distroless/static:nonroot"]
```

## Compose Rules

### Image Rules

#### `compose-pinned-images` (error)
Service images must be pinned by digest (`image: foo@sha256:...`), not just by tag.

❌ **Bad:**
```yaml
services:
  web:
    image: nginx:1.25
```

✅ **Good:**
```yaml
services:
  web:
    image: nginx:1.25@sha256:abc123...
```

#### `compose-no-latest-tag` (error)
Services must not use `:latest` tag.

❌ **Bad:**
```yaml
services:
  web:
    image: nginx:latest
```

#### `compose-no-floating-tag` (warning)
Services should not use floating major tags (`:20`, `:3`) without digest pinning.

❌ **Bad:**
```yaml
services:
  web:
    image: nginx:1
```

### Security Rules

#### `compose-no-privileged` (error)
Services must not run in privileged mode.

❌ **Bad:**
```yaml
services:
  web:
    privileged: true
```

#### `compose-security-opt` (warning)
Services should have `security_opt: ["no-new-privileges:true"]`.

✅ **Good:**
```yaml
services:
  web:
    security_opt:
      - "no-new-privileges:true"
```

#### `compose-no-docker-sock-mount` (error)
Services must not mount `/var/run/docker.sock` unless exempted by proxy label.

❌ **Bad:**
```yaml
services:
  web:
    volumes:
      - "/var/run/docker.sock:/var/run/docker.sock"
```

✅ **Good (with exempt label):**
```yaml
services:
  proxy:
    image: nginx:1.25@sha256:abc
    volumes:
      - "/var/run/docker.sock:/var/run/docker.sock"
    labels:
      com.dockerproxy.role: "proxy"
```

#### `compose-no-root-user` (warning)
Services should specify `user: <non-root>` or `user: "1000:1000"`.

✅ **Good:**
```yaml
services:
  web:
    user: "1000:1000"
```

#### `compose-no-host-network` (warning)
Services should not use `network_mode: host`.

#### `compose-no-host-pid` (warning)
Services should not use `pid: host`.

#### `compose-cap-drop` (warning)
Services should have `cap_drop: ["ALL"]` and only add back specific capabilities.

✅ **Good:**
```yaml
services:
  web:
    cap_drop:
      - "ALL"
    cap_add:
      - "NET_BIND_SERVICE"
```

#### `compose-readonly-filesystem` (info)
Long-running services should have `read_only: true` with explicit `tmpfs` for write paths.

### Health Rules

#### `compose-healthcheck` (warning)
Long-running services should define `healthcheck`.

✅ **Good:**
```yaml
services:
  web:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost"]
      interval: 30s
      timeout: 5s
      retries: 3
```

#### `compose-restart-policy` (info)
Services should define `restart: unless-stopped` or `restart: always` for production.

### Resource Rules

#### `compose-resource-limits` (warning, opt-in)
Services should define `deploy.resources.limits` (memory, cpus). Only checked
when `require_resource_limits = true` in config.

✅ **Good:**
```yaml
services:
  web:
    deploy:
      resources:
        limits:
          memory: 512M
          cpus: "0.5"
```

#### `compose-no-bind-0.0.0.0` (warning)
Port bindings should not bind to `0.0.0.0` without gateway annotation. Use
`127.0.0.1:PORT:PORT` for local-only.

❌ **Bad:**
```yaml
services:
  web:
    ports:
      - "0.0.0.0:8080:8080"
```

✅ **Good:**
```yaml
services:
  web:
    ports:
      - "127.0.0.1:8080:8080"
```

### Update Rules

#### `compose-watchtower-labels` (info, ops_mode only)
Long-running services should have watchtower/wud update labels. Only checked
when `ops_mode = true` in config.

✅ **Good:**
```yaml
services:
  web:
    labels:
      com.centurylinklabs.watchtower.enable: "true"
```

## Dockerfile Rules

### `pin-image-digests` (warning)
`FROM` instructions must be pinned by digest.

### `dockerfile-no-latest-tag` (error)
`FROM` must not use `:latest` tag or untagged images.

### `no-copy-dot` (warning)
Avoid `COPY .` — copy only required paths.

### `require-non-root-user` (warning)
Dockerfile must declare a non-root `USER`.

### `dockerfile-healthcheck` (warning)
Dockerfile should define `HEALTHCHECK` instruction.

### `dockerfile-apk-no-cache` (warning)
`apk add` commands must use `--no-cache` flag.

### `dockerfile-apt-get-no-install-recommends` (warning)
`apt-get install` must use `--no-install-recommends`.

### `dockerfile-apt-get-clean` (warning)
`apt-get install` must be followed by `rm -rf /var/lib/apt/lists/*`.

### `dockerfile-dockerignore-present` (warning)
Project with a Dockerfile should have `.dockerignore`.

### `dockerfile-multi-stage` (info)
Dockerfiles with `RUN` install commands should use multi-stage builds.

### `dockerfile-distroless-scratch-exempt`
`FROM scratch` and `gcr.io/distroless/static:nonroot` are exempt from digest
pinning requirements.

## File Types Covered

| File type | Scanner |
|-----------|---------|
| `Dockerfile` / `Dockerfile.*` / `*.dockerfile` | dockerfile_lint |
| `docker-compose.yml` / `docker-compose.yaml` | compose_lint |
| `compose.yml` / `compose.yaml` / `compose.*.yml` | compose_lint |
| `docker-compose.override.yml` | compose_lint |
| `.dockerignore` | dockerfile_lint (presence check) |

## Troubleshooting

### False Positives
1. Add proxy labels to exempt docker.sock mounts
2. Configure `exempt_from_digest_pinning` for custom distroless images
3. Disable specific rules via `[scanner_config.compose_lint]` toggles

### Performance
1. The scanner uses the centralized exclusion list — `node_modules/`, `target/`,
   `dist/` are automatically skipped
2. `serde_yaml` parsing is fast for typical compose files
