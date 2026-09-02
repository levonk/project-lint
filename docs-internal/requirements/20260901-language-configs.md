# PRD: Language Configs (pyproject.toml, go.mod, build.gradle, Cargo.toml enhancements)

**Date**: 2026-09-01
**Status**: in-progress
**Scope**: New scanners for language-specific package/config files:
`pyproject.toml` (Python), `go.mod` (Go), `build.gradle` /
`settings.gradle` (Gradle/JVM), and enhancements to the existing
`rust_conventions` scanner for `Cargo.toml` content validation.

## Problem

The existing `rust_conventions` scanner checks `Cargo.toml` for
forbidden crates and `.rs` files for `dbg!`/`unwrap()`/`unsafe`. But
there are no scanners for Python, Go, or Gradle project configs. The
scan data shows:

- 7 `pyproject.toml` files — no validation
- 4 `go.mod` files — no validation
- 2 `build.gradle` files — no validation
- 112 `Cargo.toml` files — only forbidden crate check, no content validation

## File Types Covered

| File type | Count | Scanner |
|-----------|-------|---------|
| `pyproject.toml` | ~7 | python_config |
| `go.mod` / `go.sum` | ~4 | go_config |
| `build.gradle` / `settings.gradle` | ~2 | gradle_config |
| `Cargo.toml` | ~112 | rust_conventions (enhance) |

## Rules

### python_config (check name: `python_config`) — NEW SCANNER

#### pyproject.toml rules
- [ ] `pyproject-build-system` — `pyproject.toml` should have `[build-system]` section with `requires` and `build-backend`. **Severity: error.** Auto-fixable: no.
- [ ] `pyproject-uses-uv-or-ruff` — Python projects should use `uv` for package management and `ruff` for linting (not pip/poetry/flake8). **Severity: warning.** Auto-fixable: no. **Note**: Configurable — set `required_tools = []` to disable.
- [ ] `pyproject-no-pinned-equals` — Dependencies should not use `==` pinning in `[project.dependencies]` (use `>=` ranges or lock file). **Severity: warning.** Auto-fixable: no.
- [ ] `pyproject-python-version` — `[project]` should specify `requires-python` with a minimum version. **Severity: warning.** Auto-fixable: no.
- [ ] `pyproject-ruff-config` — If using ruff, `[tool.ruff]` should be configured with `line-length` and `target-version`. **Severity: info.** Auto-fixable: no.
- [ ] `pyproject-no-setup-py` — If `pyproject.toml` exists, `setup.py` should NOT exist (legacy). **Severity: warning.** Auto-fixable: no.
- [ ] `pyproject-no-requirements-txt` — If `pyproject.toml` exists with `[project.dependencies]`, `requirements.txt` should NOT exist (use `uv.lock` instead). **Severity: info.** Auto-fixable: no.

### go_config (check name: `go_config`) — NEW SCANNER

#### go.mod rules
- [ ] `go-mod-go-version` — `go.mod` should specify a Go version (`go 1.22` or later). **Severity: warning.** Auto-fixable: no.
- [ ] `go-mod-no-replace-local` — `go.mod` should not have `replace` directives pointing to local filesystem paths (`replace foo => ../bar`). **Severity: error.** Auto-fixable: no. **Note**: Local replaces are fine in development but should not be committed.
- [ ] `go-mod-no-indirect-in-mod` — `go.mod` should not have `// indirect` dependencies in the `require` block (they belong in `go.sum`). **Severity: info.** Auto-fixable: no.
- [ ] `go-mod-tidy` — `go.mod` should be tidy (no missing dependencies). Check by running `go mod tidy -diff` if Go is available. **Severity: warning.** Auto-fixable: yes (`go mod tidy`).
- [ ] `go-sum-present` — If `go.mod` exists, `go.sum` must also exist. **Severity: error.** Auto-fixable: no.

### gradle_config (check name: `gradle_config`) — NEW SCANNER

#### build.gradle / settings.gradle rules
- [ ] `gradle-no-dynamic-versions` — Dependencies should not use `+` dynamic versions (`implementation "foo:bar:1.+"`). Pin to specific versions. **Severity: error.** Auto-fixable: no.
- [ ] `gradle-no-snapshots` — Dependencies should not use `SNAPSHOT` versions in production. **Severity: warning.** Auto-fixable: no.
- [ ] `gradle-repositories-block` — `build.gradle` should have a `repositories` block (not rely on inherited repos). **Severity: info.** Auto-fixable: no.
- [ ] `gradle-settings-plugin-management` — `settings.gradle` should have `pluginManagement` block for Gradle plugin resolution. **Severity: info.** Auto-fixable: no.
- [ ] `gradle-wrapper-present` — If `build.gradle` exists, `gradle/wrapper/gradle-wrapper.properties` should also exist. **Severity: warning.** Auto-fixable: no.
- [ ] `gradle-no-gradlew-exec` — `gradlew` should have execute permission. **Severity: warning.** Auto-fixable: yes (`chmod +x gradlew`).

### rust_conventions enhancements (check name: `rust_conventions`) — EXISTING SCANNER

#### Cargo.toml content rules (new, in addition to forbidden crates)
- [ ] `cargo-edition-2021` — `Cargo.toml` should use `edition = "2021"` (not 2018 or 2015). **Severity: warning.** Auto-fixable: no.
- [ ] `cargo-description-present` — `Cargo.toml` should have `description` field. **Severity: info.** Auto-fixable: no.
- [ ] `cargo-license-present` — `Cargo.toml` should have `license` field. **Severity: warning.** Auto-fixable: no.
- [ ] `cargo-repository-present` — `Cargo.toml` should have `repository` field. **Severity: info.** Auto-fixable: no.
- [ ] `cargo-no-floating-deps` — Dependencies should not use `*` or unbounded `>=` version specs. **Severity: warning.** Auto-fixable: no.
- [ ] `cargo-workspace-root-deps` — Workspace root `Cargo.toml` should use `[workspace.dependencies]` for shared deps, not per-crate duplication. **Severity: info.** Auto-fixable: no.
- [ ] `cargo-no-criterion-bench-in-dev-deps` — `criterion` should be in `[dev-dependencies]`, not `[dependencies]`. **Severity: warning.** Auto-fixable: no.

## Implementation

### PythonConfigScanner, GoConfigScanner, GradleConfigScanner

All parse their respective files as text/TOML/Gradle DSL and check
patterns. `pyproject.toml` is TOML — use `toml` crate if available,
otherwise regex. `go.mod` is a simple text format — line parsing.
`build.gradle` is Groovy DSL — regex/line parsing (not full Groovy
parsing).

### RustConventionsScanner enhancement

Add `scan_cargo_toml()` rules for the new checks. The existing
`scan_cargo_toml()` already checks forbidden crates — extend it.

## Configuration

```toml
[scanner_config.python_config]
required_tools = ["uv", "ruff"]
forbid_setup_py = true
forbid_requirements_txt = false

[scanner_config.go_config]
require_go_sum = true
forbid_local_replace = true
check_tidy = false  # requires Go installed

[scanner_config.gradle_config]
forbid_dynamic_versions = true
forbid_snapshots = true
require_wrapper = true

[scanner_config.rust_security]
# Existing
forbidden_crates = []
# New
require_edition_2021 = true
require_license = true
forbid_floating_deps = true
```

## Acceptance Criteria

- [ ] All three new scanners exist with `scan()` returning `Vec<ScannerIssue>`
- [ ] All three registered in `mod.rs`, wired in `lint.rs`, config in `config.rs`, documented in `AGENTS.md`
- [ ] `rust_conventions` enhanced with new Cargo.toml rules
- [ ] All scanners use centralized exclusion list
- [ ] Tests for each rule
- [ ] Smoke test: silent on repos without these files
- [ ] Smoke test: fires on `buzz` (has SQL + possibly Python), `devbox`/`deptrust`/`treehouse` (go.mod), `mrepo` (build.gradle), any Rust repo (Cargo.toml)
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **Python runtime checks** — no `python -c` execution or import validation.
- **Go test/build** — no `go build` or `go test` execution.
- **Gradle build** — no `gradle build` execution.
- **Maven** — `pom.xml` not covered (0 found in scan). Future scanner.
- **CMake** — `CMakeLists.txt` not covered. Future scanner.
- **Bazel** — `BUILD` / `BUILD.bazel` / `WORKSPACE` not covered. Future scanner.
- **Zig** — `build.zig` not covered. Future scanner.
- **Swift** — `Package.swift` not covered. Future scanner.
- **Mix/Elixir** — `mix.exs` not covered. Future scanner.

## Dependencies

- **Centralized exclusion list** — must not scan `node_modules/`, `target/`, `vendor/` (Go).
- **`toml` crate** — for parsing `pyproject.toml` and `Cargo.toml`. Check if already in dependencies.
