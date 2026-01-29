# Dependency Version Checker

This module provides dependency version checking capabilities for project-lint.

## Features

- **Multi-language support**: Checks dependencies for npm, Cargo, pip/poetry, and more
- **Version comparison**: Identifies major, minor, and patch version differences
- **Auto-update**: Can automatically update dependency versions in configuration files
- **Configurable warnings**: Different severity levels for different version types

## Supported Package Managers

- **npm/yarn/pnpm**: `package.json` files
- **Cargo**: `Cargo.toml` files  
- **pip**: `requirements.txt` files
- **Poetry**: `pyproject.toml` files
- **Maven**: `pom.xml` files (placeholder)
- **Gradle**: `build.gradle` files (placeholder)
- **Composer**: `composer.json` files (placeholder)
- **Bundler**: `Gemfile` files (placeholder)
- **Mix**: `mix.exs` files (placeholder)
- **Go**: `go.mod` files (placeholder)
- **Nix**: `flake.nix` files (placeholder)

## Usage

The dependency checker is integrated into the main project-lint workflow:

```bash
project-lint lint  # Will check dependencies if enabled
```

## Configuration

Add to `.config/project-lint/rules/active/dependency-versions.toml`:

```toml
name = "dependency-versions"
description = "Check for outdated dependencies"
enabled = true
severity = "warning"

[dependency_checker]
check_outdated = true
warn_major_versions = true
warn_minor_versions = true
warn_patch_versions = false
```

## Output Examples

```
🔴 [Dependencies] react is significantly outdated (MAJOR) (16.14.0 -> 18.2.0) via npm (package.json)
🟡 [Dependencies] express has a minor update available (minor) (4.17.1 -> 4.18.2) via npm (package.json)
🟢 [Dependencies] lodash has a patch update available (patch) (4.17.21 -> 4.17.22) via npm (package.json)
```

## Implementation Notes

- Uses external CLI tools (`npm`, `cargo`, `pip`) to query latest versions
- Preserves version specifiers (^, ~, >=, etc.) when auto-updating
- Supports dry-run mode to preview changes before applying them
- Handles multiple package manager files in the same project
