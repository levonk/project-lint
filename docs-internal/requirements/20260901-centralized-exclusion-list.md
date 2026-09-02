# PRD: Centralized Exclusion List

**Date**: 2026-09-01
**Status**: proposed
**Scope**: Create a shared utility that all `WalkDir`-based scanners use
to skip build artifacts, dependency directories, and VCS internals.
This is a prerequisite for all content-validation scanners — without
it, scanners will produce false positives by scanning `node_modules/`,
`target/`, `dist/`, etc.

## Problem

Each scanner currently implements its own path filtering (or doesn't
filter at all). The existing approaches are inconsistent:

- `rust_conventions.rs` — checks `rel_str.starts_with("target/")` and `.git/`
- `dockerfile_lint.rs` — uses `WalkDir::max_depth(4)` but no path filtering
- `magic_numbers.rs` — has its own skip list
- `config_validation.rs` (unwired) — no filtering at all, would scan every `package.json` in `node_modules/`

The scan data shows ~2500 `package.json` files across the two repo
trees, but only ~50 are first-party. Without a centralized exclusion
list, the `config_validation` scanner would emit thousands of false
positives from dependency `package.json` files.

## File Types Covered

This is not a file-type scanner — it's a cross-cutting utility that
all scanners use. It filters directory paths during `WalkDir` traversal.

## Rules

### Excluded directories (always skipped)
- [ ] `exclusion-node-modules` — Skip `node_modules/` directory and all contents. **Severity: N/A (structural).** Auto-fixable: no.
- [ ] `exclusion-target` — Skip `target/` (Rust build output). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-dist` — Skip `dist/` (JS build output). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-build` — Skip `build/` (generic build output, Go builds, etc.). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-next` — Skip `.next/` (Next.js build cache). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-turbo` — Skip `.turbo/` (Turborepo cache). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-nuxt` — Skip `.nuxt/` (Nuxt build cache). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-svelte-kit` — Skip `.svelte-kit/` (SvelteKit build cache). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-git` — Skip `.git/` (VCS internals). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-vendor` — Skip `vendor/` (Go vendored dependencies). **Severity: N/A.** Auto-fixable: no. **Note**: This is configurable — some projects have first-party `vendor/` dirs.
- [ ] `exclusion-devbox-gen` — Skip `.devbox/gen/` (devbox generated files). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-cache` — Skip `.cache/` (generic cache). **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-coverage` — Skip `coverage/` (test coverage output). **Severity: N/A.** Auto-fixable: no.

### Excluded files (by pattern)
- [ ] `exclusion-lockfiles-from-content-scanners` — Lockfiles (`*.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `Cargo.lock`, `flake.lock`, `devbox.lock`, `uv.lock`, `poetry.lock`) are excluded from content scanners but NOT from presence-check scanners. **Severity: N/A.** Auto-fixable: no.

### Configurable exclusions
- [ ] `exclusion-config-extra-exclude` — Projects can add extra excluded paths via `[scanner_config.exclusion] extra_excludes = ["path1", "path2"]`. **Severity: N/A.** Auto-fixable: no.
- [ ] `exclusion-config-allow-vendor` — Projects with first-party `vendor/` can set `allow_vendor = true` to not skip it. **Severity: N/A.** Auto-fixable: no.

## Implementation

### Shared utility in `project-lint-core/src/utils.rs`

```rust
/// Default directories excluded from all WalkDir-based scanners.
pub const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".nuxt",
    ".svelte-kit",
    ".git",
    ".devbox/gen",
    ".cache",
    "coverage",
];

/// Filter function for WalkDir — returns true if the entry should be
/// traversed, false if it should be skipped.
pub fn is_excluded_dir(path: &Path, excluded: &[String]) -> bool {
    // Check each component of the path against the excluded list
    path.components().any(|comp| {
        let comp_str = comp.as_os_str().to_string_lossy();
        excluded.iter().any(|ex| {
            ex == &*comp_str || comp_str.starts_with(&format!("{}/", ex))
        })
    })
}

/// Create a WalkDir with standard exclusions applied via filter_entry.
pub fn walk_project(root: &Path, extra_excludes: &[String]) -> WalkDir {
    let mut excluded: Vec<String> = DEFAULT_EXCLUDED_DIRS
        .iter().map(|s| s.to_string()).collect();
    excluded.extend(extra_excludes.iter().cloned());
    WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|e| !is_excluded_dir(e.path(), &excluded))
        // ... (but filter_entry is on the iterator, not WalkDir itself)
}
```

### Migration plan

Each existing scanner that does its own filtering should be updated to
use the shared utility. This is a refactor, not a behavior change —
the scanner's output should be identical before and after.

1. Add the utility to `utils.rs`
2. Update `rust_conventions.rs` to use it (remove inline `target/` check)
3. Update `magic_numbers.rs` to use it (remove inline skip list)
4. Update `dockerfile_lint.rs` to use it (add filtering where there was none)
5. Update `skill_markdown.rs` to use it
6. Update `submodule_integrity.rs` to use it
7. All new scanners (from other PRDs) use it from the start

## Configuration

```toml
[scanner_config.exclusion]
extra_excludes = ["my-build-dir", "generated"]
allow_vendor = false  # set true to NOT exclude vendor/
max_depth = 4         # WalkDir max depth (default 4)
```

## Acceptance Criteria

- [ ] `is_excluded_dir()` function exists in `project-lint-core/src/utils.rs`
- [ ] `walk_project()` helper exists (or equivalent WalkDir builder)
- [ ] All existing scanners that use `WalkDir` are updated to use the shared utility
- [ ] Unit tests verify each excluded directory is correctly skipped
- [ ] Unit test verifies `extra_excludes` are applied
- [ ] Unit test verifies `allow_vendor = true` does NOT exclude `vendor/`
- [ ] Smoke test: `config_validation` scanner (when wired) does NOT scan `node_modules/` package.json files
- [ ] Smoke test: `rust_conventions` scanner output is identical before and after migration
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **File-level exclusions** (e.g., skipping specific files by name) — handled by individual scanners or modular rules, not the centralized list.
- **`.gitignore` parsing** — the exclusion list is hardcoded + configurable, not derived from `.gitignore`. A future enhancement could read `.gitignore` but it adds complexity and most build artifacts are already covered.
- **Symlink following** — `WalkDir` does not follow symlinks by default. The `node_modules` symlink integrity scanner (separate PRD) handles symlink validation explicitly.

## Dependencies

- None — this is a foundational utility that all other PRDs depend on.
