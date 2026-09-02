# PRD: Binary Validation Scanner

**Date**: 2026-09-01
**Status**: proposed
**Scope**: Create a new scanner that validates binary files committed to a
repository — detects binaries that should be tracked via Git LFS, oversized
binary files, binaries committed into source directories, and binaries that
duplicate assets already present elsewhere in the tree.

## Problem

Binary files (images, PDFs, videos, archives, compiled blobs) bloat git
history when committed directly instead of through Git LFS. Common problems
this scanner addresses:

- A 12 MB PNG committed into `src/assets/` — every clone pays the cost forever.
- A `.mp4` demo video checked into the repo root instead of an external store.
- A `.zip` / `.tar.gz` build artifact committed alongside source.
- A `.pdf` documentation asset that should live in an external docs store.
- Duplicate binaries (the same `logo.png` copied into three package dirs).

Without a scanner, these slip in during normal development and are expensive
to remove after they are part of history.

## File Types Covered

The scanner inspects files by extension. The default set covers common binary
asset types:

- **Images**: `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.tiff`, `.webp`, `.ico`, `.psd`
- **Video**: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`
- **Audio**: `.mp3`, `.wav`, `.flac`, `.aac`, `.ogg`
- **Documents**: `.pdf`, `.doc`, `.docx`, `.xls`, `.xlsx`, `.ppt`, `.pptx`
- **Archives**: `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.gz`, `.bz2`, `.7z`, `.rar`
- **Compiled**: `.so`, `.dll`, `.dylib`, `.exe`, `.o`, `.a`, `.class`, `.jar`, `.wasm`
- **Fonts**: `.ttf`, `.otf`, `.woff`, `.woff2` (only flagged when oversized or in source dirs)

`.svg` is text (XML) but is treated as a binary asset for the LFS / source-dir
rules because it is an image format.

## Rules

### `binary-lfs-required`
- [ ] `binary-lfs-required` — A binary file exceeding the LFS threshold
  (default 1 MB) is committed directly to git. It should be tracked via Git
  LFS instead. **Severity: warning.** Auto-fixable: no.

### `binary-oversized`
- [ ] `binary-oversized` — A binary file exceeds the hard size limit (default
  10 MB) regardless of LFS status. Files this large should not be in the
  repository at all — use an external asset store. **Severity: error.**
  Auto-fixable: no.

### `binary-in-source-dir`
- [ ] `binary-in-source-dir` — A binary file is located inside a source
  directory (`src/`, `lib/`, `cmd/`, `internal/`, `app/`, `packages/*/src/`).
  Binaries belong in dedicated asset directories (`assets/`, `public/`,
  `static/`, `resources/`, `media/`), not next to source code. **Severity:
  warning.** Auto-fixable: no.

### `binary-duplicate`
- [ ] `binary-duplicate` — Two or more binary files have identical content
  (same SHA-256) but different paths. Duplicates waste repository space and
  create maintenance drift. **Severity: warning.** Auto-fixable: no.

### `binary-archive-committed`
- [ ] `binary-archive-committed` — A build artifact archive (`.zip`, `.tar`,
  `.tar.gz`, `.tgz`, `.gz`, `.bz2`, `.7z`, `.rar`) is committed to the
  repository. Archives are build outputs and should be produced by the build
  system, not checked in. **Severity: warning.** Auto-fixable: no.

### `binary-compiled-artifact`
- [ ] `binary-compiled-artifact` — A compiled artifact (`.so`, `.dll`,
  `.dylib`, `.exe`, `.o`, `.a`, `.class`, `.jar`, `.wasm`) is committed to the
  repository. Compiled artifacts are build outputs. **Severity: error.**
  Auto-fixable: no.

## Configuration

```toml
[scanner_config.binary_validation]
# Files larger than this (in bytes) should use Git LFS. Default 1 MB.
lfs_threshold_bytes = 1048576
# Hard size limit (in bytes) — files larger than this are errors. Default 10 MB.
max_size_bytes = 10485760
# When true, emit binary-lfs-required for files over lfs_threshold_bytes.
check_lfs = true
# When true, emit binary-oversized for files over max_size_bytes.
check_oversized = true
# When true, emit binary-in-source-dir for binaries in source directories.
check_source_dir = true
# When true, emit binary-duplicate for identical-content binaries.
check_duplicate = true
# When true, emit binary-archive-committed for committed archives.
check_archive = true
# When true, emit binary-compiled-artifact for committed compiled artifacts.
check_compiled = true
# Extra file extensions to treat as binary (e.g. ["dat", "bin"]).
extra_binary_extensions = []
# Directory names considered source directories (in addition to defaults).
extra_source_dirs = []
```

## Acceptance Criteria

- [ ] `BinaryValidationScanner` struct exists in
  `project-lint-core/src/scanners/binary_validation.rs` with `new()`,
  `with_config()`, `with_exclusions()`, `scan()`, and `impl Default`.
- [ ] Scanner uses `ScannerIssue` from `project-lint-core/src/scanners/mod.rs`.
- [ ] Scanner uses the centralized exclusion helper (`build_exclusions` /
  `walk_project` / `is_excluded_rel`) from `project-lint-core/src/utils.rs`.
- [ ] Module registered in `project-lint-core/src/scanners/mod.rs`.
- [ ] Scanner wired into `src/commands/lint.rs` gated by the
  `binary_validation` check name.
- [ ] Config struct `BinaryValidationConfig` added to
  `project-lint-core/src/config.rs` with a `ScannerConfig` field and `Default`
  impl.
- [ ] `AGENTS.md` Analysis Modules section updated.
- [ ] Colocated `mod tests` covers each rule: positive (flags violation),
  negative (clean file passes), and edge case (config disables a check,
  exclusion list skips a dir).
- [ ] `devbox run -- just quality` passes.
- [ ] `devbox run -- just quality-full` passes.
- [ ] `docs/lint-categories/binary.md` created following the format of
  existing category docs.
- [ ] `docs-internal/implementation-summary.md` updated.
- [ ] Smoke test documented in
  `docs-internal/requirements/testing/20260902-binary-validation-smoke.md`.

## Out of Scope

- **Git LFS attribute verification** — checking `.gitattributes` for
  `filter=lfs` entries is a future enhancement; the current scanner flags by
  size threshold only.
- **Image dimension / format validation** — checking that a PNG is the
  declared resolution or format is out of scope (a separate image-lint scanner
  would handle that).
- **Auto-fixing** — the scanner does not move or delete binaries; it only
  reports. Auto-fix for binaries is destructive and out of scope.
- **SVG content linting** — SVG is XML; content rules (inline scripts,
  metadata) belong to a markup scanner, not this one.

## Dependencies

- Depends on the centralized exclusion list (`20260901-centralized-exclusion-list.md`)
  for WalkDir filtering.
