# Language Config Rules

Language config rules validate language-specific package and configuration files for modern conventions and best practices.

## Overview

Language config rules help identify:
- Missing or incomplete build system configuration
- Outdated language versions
- Legacy packaging patterns (setup.py, requirements.txt)
- Dynamic or floating dependency versions
- Missing project metadata (license, description, repository)
- Improper dependency placement (criterion in deps vs dev-deps)

## Scanners

### Python Config (`python_config`)

Validates `pyproject.toml` for modern Python packaging conventions.

**Check name**: `python_config`
**Scanner**: `project-lint-core/src/scanners/python_config.rs`

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `pyproject-build-system` | error | `[build-system]` section with `requires` and `build-backend` |
| `pyproject-uses-uv-or-ruff` | warning | `uv` and `ruff` tool sections present |
| `pyproject-no-pinned-equals` | warning | No `==` pinning in `[project.dependencies]` |
| `pyproject-python-version` | warning | `requires-python` field present |
| `pyproject-ruff-config` | info | `[tool.ruff]` has `line-length` and `target-version` |
| `pyproject-no-setup-py` | warning | No `setup.py` alongside `pyproject.toml` |
| `pyproject-no-requirements-txt` | info | No `requirements.txt` with `[project.dependencies]` |

#### Configuration

```toml
[scanner_config.python_config]
required_tools = ["uv", "ruff"]
forbid_setup_py = true
forbid_requirements_txt = false
```

### Go Config (`go_config`)

Validates `go.mod` / `go.sum` for Go module conventions.

**Check name**: `go_config`
**Scanner**: `project-lint-core/src/scanners/go_config.rs`

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `go-mod-go-version` | warning | Go version 1.22+ specified |
| `go-mod-no-replace-local` | error | No local filesystem `replace` directives |
| `go-mod-no-indirect-in-mod` | info | No `// indirect` in `go.mod` require block |
| `go-sum-present` | error | `go.sum` exists when `go.mod` exists |

#### Configuration

```toml
[scanner_config.go_config]
require_go_sum = true
forbid_local_replace = true
```

### Gradle Config (`gradle_config`)

Validates `build.gradle` / `settings.gradle` for Gradle/JVM conventions.

**Check name**: `gradle_config`
**Scanner**: `project-lint-core/src/scanners/gradle_config.rs`

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `gradle-no-dynamic-versions` | error | No `+` dynamic version specifiers |
| `gradle-no-snapshots` | warning | No `SNAPSHOT` versions |
| `gradle-repositories-block` | info | `repositories` block present |
| `gradle-settings-plugin-management` | info | `pluginManagement` in settings.gradle |
| `gradle-wrapper-present` | warning | `gradle-wrapper.properties` present |
| `gradle-no-gradlew-exec` | warning | `gradlew` has execute permission |

#### Configuration

```toml
[scanner_config.gradle_config]
forbid_dynamic_versions = true
forbid_snapshots = true
require_wrapper = true
```

### Rust Conventions (`rust_conventions`)

Enhanced existing scanner with Cargo.toml content validation.

**Check name**: `rust_conventions`
**Scanner**: `project-lint-core/src/scanners/rust_conventions.rs`

#### Cargo.toml Rules (new)

| Rule | Severity | Description |
|------|----------|-------------|
| `cargo-edition-2021` | warning | `edition = "2021"` |
| `cargo-description-present` | info | `description` field present |
| `cargo-license-present` | warning | `license` field present |
| `cargo-repository-present` | info | `repository` field present |
| `cargo-no-floating-deps` | warning | No `*` wildcard versions |
| `cargo-workspace-root-deps` | info | Workspace root uses `[workspace.dependencies]` |
| `cargo-no-criterion-bench-in-dev-deps` | warning | `criterion` in `[dev-dependencies]` |

#### Configuration

```toml
[scanner_config.rust_security]
forbidden_crates = []
require_edition_2021 = true
require_license = true
forbid_floating_deps = true
```

## Enabling Scanners

All four scanners are gated by their check names. To enable them, add the check names to your `enabled_checks` list or ensure they are not in `disabled_checks`:

```toml
[rules]
enabled_checks = [
    "python_config",
    "go_config",
    "gradle_config",
    "rust_conventions",
]
```

Scanners are silent when no matching files exist — they will not produce false positives on repos that don't use the target language.

## Exclusion List

All scanners use the centralized exclusion list (`[scanner_config.exclusion]`) to skip build artifacts, dependency directories, and VCS internals (`node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `.turbo/`, `.git/`, etc.).
