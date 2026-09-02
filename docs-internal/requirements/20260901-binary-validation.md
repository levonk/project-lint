# PRD: Binary File Validation (PNG, JPG, SVG, PDF, MP4, generic format checks)

**Date**: 2026-09-01
**Status**: proposed
**Scope**: New scanner for validating that binary files are of the
correct format — detecting corrupted files, misnamed extensions, and
files with embedded malicious content. The scan data shows 345 PNG,
244 JPG, 126 SVG, 658 JPG, and 21 MP4 files across repos.

## Problem

Binary files can be:
1. **Corrupted** — truncated downloads, incomplete writes
2. **Misnamed** — a `.png` file that's actually a JPEG
3. **Malicious** — an SVG containing `<script>` tags (XSS vector), a
   PDF with embedded JavaScript, an image with embedded EXIF metadata
   containing GPS coordinates or personal info

No binary validation exists in project-lint. The scan data shows
hundreds of binary files with no format validation.

## File Types Covered

| File type | Count | Scanner |
|-----------|-------|---------|
| `*.png` | ~345 | binary_validation |
| `*.jpg` / `*.jpeg` | ~658 | binary_validation |
| `*.gif` | ~46 | binary_validation |
| `*.svg` | ~126 | binary_validation |
| `*.pdf` | ~17 | binary_validation |
| `*.mp4` / `*.webm` / `*.mov` | ~21 | binary_validation |
| `*.ico` | ~5 | binary_validation |
| `*.bmp` | ~2 | binary_validation |
| `*.webp` | ~3 | binary_validation |

## Rules

### binary_validation (check name: `binary_validation`) — NEW SCANNER

#### Magic number / format validation
- [ ] `binary-png-magic` — `.png` files must start with magic bytes `89 50 4E 47 0D 0A 1A 0A` (PNG signature). **Severity: error.** Auto-fixable: no.
- [ ] `binary-jpg-magic` — `.jpg`/`.jpeg` files must start with `FF D8 FF` (JPEG SOI marker). **Severity: error.** Auto-fixable: no.
- [ ] `binary-gif-magic` — `.gif` files must start with `GIF87a` or `GIF89a`. **Severity: error.** Auto-fixable: no.
- [ ] `binary-pdf-magic` — `.pdf` files must start with `%PDF-`. **Severity: error.** Auto-fixable: no.
- [ ] `binary-mp4-magic` — `.mp4` files must have `ftyp` box at offset 4. **Severity: error.** Auto-fixable: no.
- [ ] `binary-webp-magic` — `.webp` files must start with `RIFF` and contain `WEBP` at offset 8. **Severity: error.** Auto-fixable: no.
- [ ] `binary-ico-magic` — `.ico` files must start with `00 00 01 00`. **Severity: error.** Auto-fixable: no.
- [ ] `binary-bmp-magic` — `.bmp` files must start with `BM`. **Severity: error.** Auto-fixable: no.
- [ ] `binary-webm-magic` — `.webm` files must start with `1A 45 DF A3` (EBML header). **Severity: error.** Auto-fixable: no.

#### Extension mismatch detection
- [ ] `binary-extension-mismatch` — File extension does not match actual format (e.g., `.png` file with JPEG magic bytes). **Severity: error.** Auto-fixable: yes (rename extension to match actual format).

#### SVG-specific rules (SVG is text/XML, not binary)
- [ ] `svg-no-script-tags` — SVG files must not contain `<script>` tags (XSS vector). **Severity: error.** Auto-fixable: no.
- [ ] `svg-no-event-handlers` — SVG files must not contain inline event handlers (`onload=`, `onclick=`, `onerror=`). **Severity: error.** Auto-fixable: no.
- [ ] `svg-no-external-references` — SVG files should not reference external URLs (`xlink:href="http://..."`). **Severity: warning.** Auto-fixable: no.
- [ ] `svg-xml-valid` — SVG files must be valid XML (well-formed). **Severity: error.** Auto-fixable: no.
- [ ] `svg-has-xmlns` — SVG root element should declare `xmlns="http://www.w3.org/2000/svg"`. **Severity: warning.** Auto-fixable: no.
- [ ] `svg-no-dimensions-in-style` — SVG should use `width`/`height` attributes, not `style="width:..."` for sizing. **Severity: info.** Auto-fixable: no.

#### PDF-specific rules
- [ ] `pdf-no-javascript` — PDF files should not contain embedded JavaScript (`/JavaScript` or `/JS` in PDF structure). **Severity: error.** Auto-fixable: no.
- [ ] `pdf-no-embedded-files` — PDF files should not contain embedded files (`/EmbeddedFile` in PDF structure). **Severity: warning.** Auto-fixable: no.
- [ ] `pdf-no-launch-action` — PDF files should not contain `/Launch` actions (can execute arbitrary commands). **Severity: error.** Auto-fixable: no.

#### EXIF / metadata rules (for JPEG/PNG)
- [ ] `binary-no-gps-exif` — JPEG/PNG files should not contain GPS coordinates in EXIF metadata. **Severity: warning.** Auto-fixable: yes (strip EXIF). **Note**: Requires EXIF parsing library.
- [ ] `binary-no-personal-exif` — JPEG/PNG files should not contain personal metadata (camera serial, owner name). **Severity: info.** Auto-fixable: yes (strip EXIF).

#### File size rules
- [ ] `binary-file-size-reasonable` — Binary files should not exceed a configurable size limit (default: 10MB for images, 100MB for videos). **Severity: warning.** Auto-fixable: no. **Note**: Large binaries bloat the repo; use Git LFS.

## Implementation

### BinaryValidationScanner (new file: `project-lint-core/src/scanners/binary_validation.rs`)

```rust
pub struct BinaryValidationScanner {
    check_magic_numbers: bool,
    check_extension_mismatch: bool,
    check_svg_security: bool,
    check_pdf_security: bool,
    check_exif: bool,
    max_image_size_mb: u64,
    max_video_size_mb: u64,
}
```

The scanner walks the project for binary files, reads the first few
bytes (magic number check), and validates format. SVG files are read
as text and checked for `<script>`, event handlers, external refs.
PDF files are scanned for `/JavaScript`, `/JS`, `/Launch`,
`/EmbeddedFile` patterns in the binary stream.

**Magic number checking**: Read first 16 bytes of each file, compare
against known signatures. No external crate needed — just byte
comparison.

**SVG checking**: Read as UTF-8 text, use regex for `<script>`,
`onload=`, `onclick=`, `onerror=`, `xlink:href="http`.

**PDF checking**: Read as bytes, search for `/JavaScript`, `/JS`,
`/Launch`, `/EmbeddedFile` byte patterns.

**EXIF checking**: Optional, requires an EXIF parsing crate
(`kamadak-exif` or similar). Gate behind a config flag — default off
since it adds a dependency.

---

## LFS / Size / Source-Dir / Duplicate Rules (implemented)

The following rules are implemented in the current `binary_validation`
scanner and focus on repository hygiene rather than format validation.

### Problem

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

### File Types Covered

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

### Rules

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
# --- Format validation config (future) ---
check_magic_numbers = true
check_extension_mismatch = true
check_svg_security = true
check_pdf_security = true
check_exif = false  # requires exif crate
max_image_size_mb = 10
max_video_size_mb = 100

# --- LFS / size / source-dir / duplicate config (implemented) ---
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

### Format validation (future)
- [ ] `BinaryValidationScanner` exists with `scan()` returning `Vec<ScannerIssue>`
- [ ] Registered in `mod.rs`, wired in `lint.rs`, config in `config.rs`, documented in `AGENTS.md`
- [ ] Magic number validation works for all listed formats (PNG, JPG, GIF, PDF, MP4, WebP, ICO, BMP, WebM)
- [ ] Extension mismatch detection works (`.png` with JPEG content is flagged)
- [ ] SVG security checks detect `<script>` tags and event handlers
- [ ] PDF security checks detect embedded JavaScript and launch actions
- [ ] Scanner is silent on repos with no binary files
- [ ] Scanner uses centralized exclusion list (must not scan `node_modules/` images, `target/` artifacts)
- [ ] Tests for each format's magic number
- [ ] Tests for SVG security rules
- [ ] Tests for extension mismatch
- [ ] Smoke test: fires on repos with images (most repos have PNG/SVG in docs or assets)
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

### LFS / size / source-dir / duplicate (implemented)
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

### Format validation
- **Image optimization** — compressing images is not a linting concern. Use `oxipng` / `pngquant` separately.
- **Video transcoding** — validating video codec, resolution, bitrate is out of scope.
- **Audio files** — `.mp3`, `.wav`, `.ogg` not covered in initial version. Future enhancement.
- **Font files** — `.woff`, `.woff2`, `.ttf`, `.otf` not covered. Future enhancement.
- **Archive files** — `.zip`, `.tar`, `.gz` not covered. Future enhancement.
- **Steganography detection** — detecting hidden data in images is out of scope for a linter.

### LFS / size / source-dir / duplicate
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

### Format validation
- **Centralized exclusion list** — CRITICAL. Must not scan `node_modules/` (contains thousands of images), `target/`, `.next/` (Next.js image cache), `.turbo/`.
- **Optional: `kamadak-exif` crate** — only if `check_exif = true`. Don't add as hard dependency.

### LFS / size / source-dir / duplicate
- Depends on the centralized exclusion list (`20260901-centralized-exclusion-list.md`)
  for WalkDir filtering.
