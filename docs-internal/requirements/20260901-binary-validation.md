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

## Configuration

```toml
[scanner_config.binary_validation]
check_magic_numbers = true
check_extension_mismatch = true
check_svg_security = true
check_pdf_security = true
check_exif = false  # requires exif crate
max_image_size_mb = 10
max_video_size_mb = 100
```

## Acceptance Criteria

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

## Out of Scope

- **Image optimization** — compressing images is not a linting concern. Use `oxipng` / `pngquant` separately.
- **Video transcoding** — validating video codec, resolution, bitrate is out of scope.
- **Audio files** — `.mp3`, `.wav`, `.ogg` not covered in initial version. Future enhancement.
- **Font files** — `.woff`, `.woff2`, `.ttf`, `.otf` not covered. Future enhancement.
- **Archive files** — `.zip`, `.tar`, `.gz` not covered. Future enhancement.
- **Steganography detection** — detecting hidden data in images is out of scope for a linter.

## Dependencies

- **Centralized exclusion list** — CRITICAL. Must not scan `node_modules/` (contains thousands of images), `target/`, `.next/` (Next.js image cache), `.turbo/`.
- **Optional: `kamadak-exif` crate** — only if `check_exif = true`. Don't add as hard dependency.
