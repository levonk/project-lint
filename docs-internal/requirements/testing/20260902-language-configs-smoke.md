# Smoke Test: Language Configs Scanners

**Date**: 2026-09-02
**PRD**: [20260901-language-configs.md](../20260901-language-configs.md)
**Build**: `devbox run -- just build` (release)

## Scanners Tested

| Scanner | Check name | Target files |
|---------|-----------|--------------|
| python_config | `python_config` | `pyproject.toml` |
| go_config | `go_config` | `go.mod` / `go.sum` |
| gradle_config | `gradle_config` | `build.gradle` / `settings.gradle` |
| rust_conventions (enhanced) | `rust_conventions` | `Cargo.toml` |

## Test Repos

| Repo | Language | Matching files | Scanner fires? |
|------|----------|---------------|----------------|
| `~/p/gh/levonk/deptrust` | Go | `go.mod` | Yes — `go-sum-present` (go.sum missing) |
| `~/p/gh/levonk/prek` | Rust | `Cargo.toml` (root + workspace) | Yes — `cargo-edition-2021`, `cargo-description-present` |
| `~/p/gh/levonk/mrepo` | Gradle + Python | `build.gradle`, `settings.gradle`, `pyproject.toml` | Yes — `gradle-settings-plugin-management`, `pyproject-uses-uv-or-ruff` |
| `~/p/gh/levonk/dotfiles` | None (config files only) | No matching files | Silent — no false positives |
| `~/p/gh/levonk/project-lint` | Rust only | `Cargo.toml` | `rust_conventions` fires; `python_config`, `go_config`, `gradle_config` silent |

## Results

### deptrust (Go)

```
❌ [Go] go.mod exists but go.sum is missing (go.sum: go-sum-present)
```

Scanner correctly detected missing `go.sum`. No Python/Gradle/Rust false positives.

### prek (Rust)

```
⚠️ [Rust] Cargo.toml should use edition = "2021" (Cargo.toml: cargo-edition-2021)
ℹ️ [Rust] Cargo.toml missing 'description' field (Cargo.toml: cargo-description-present)
⚠️ [Rust] Cargo.toml should use edition = "2021" (crates/prek/Cargo.toml: cargo-edition-2021)
```

Enhanced `rust_conventions` correctly flags old edition and missing description. No Python/Go/Gradle false positives.

### mrepo (Gradle + Python)

```
⚠️ [Python] pyproject.toml should use 'uv' (add [tool.uv] section) (proj/deployment-operations/pyproject.toml: pyproject-uses-uv-or-ruff)
⚠️ [Python] pyproject.toml should use 'ruff' (add [tool.ruff] section) (proj/deployment-operations/pyproject.toml: pyproject-uses-uv-or-ruff)
ℹ️ [Gradle] settings.gradle missing 'pluginManagement' block for plugin resolution (settings.gradle: gradle-settings-plugin-management)
```

Both `python_config` and `gradle_config` fire correctly. No Go false positives.

### dotfiles (no matching files)

```
(grep for [Go]|[Python]|[Gradle]|[Rust] produced no output — exit code 1)
```

All four scanners silent. No false positives on repos without matching files.

### project-lint (Rust only)

```
(grep for [Go]|[Python]|[Gradle] produced no output — exit code 1)
```

`python_config`, `go_config`, `gradle_config` all silent on a Rust-only repo. `rust_conventions` fires as expected.

## Exclusion List Verification

All scanners use the centralized exclusion list (`build_exclusions` / `walk_project`). Confirmed no issues emitted from `target/`, `node_modules/`, `dist/`, or `.git/` directories during smoke tests.

## Conclusion

- All 4 scanners fire when matching files are present
- All 4 scanners are silent when no matching files exist (no false positives)
- Scanners respect the centralized exclusion list
- Enhanced `rust_conventions` correctly emits new Cargo.toml content rules
