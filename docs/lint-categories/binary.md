# Binary Validation Rules

Binary validation rules detect binary files committed to a repository that
should be tracked via Git LFS, are oversized, live in source directories,
duplicate other binaries, or are build artifacts (archives / compiled blobs)
that should never be checked in.

## Configuration

```toml
[scanner_config.binary_validation]
# Files larger than this (in bytes) should use Git LFS. Default 1 MB.
lfs_threshold_bytes = 1048576
# Hard size limit (in bytes) — files larger than this are errors. Default 10 MB.
max_size_bytes = 10485760
# Per-rule toggles (all default to true).
check_lfs = true
check_oversized = true
check_source_dir = true
check_duplicate = true
check_archive = true
check_compiled = true
# Extra file extensions to treat as binary (e.g. ["dat", "bin"]).
extra_binary_extensions = []
# Extra directory names to treat as source directories.
extra_source_dirs = []
```

Enable the scanner via the `binary_validation` check name:

```toml
[rules]
enabled_checks = ["binary_validation"]
```

## Rules

- **`binary-lfs-required`** (warning): A binary file exceeds the LFS threshold
  (default 1 MB). Track it via Git LFS instead of committing directly.
- **`binary-oversized`** (error): A binary file exceeds the hard size limit
  (default 10 MB). Use an external asset store.
- **`binary-in-source-dir`** (warning): A binary file is inside a source
  directory (`src/`, `lib/`, `cmd/`, `internal/`, `app/`, `core/`, `bin/`).
  Move it to an asset directory (`assets/`, `public/`, `static/`,
  `resources/`, `media/`, `img/`, `images/`, `fonts/`).
- **`binary-duplicate`** (warning): Two or more binary files share identical
  content (same SHA-256). Remove the duplicate.
- **`binary-archive-committed`** (warning): A build archive (`.zip`, `.tar`,
  `.tar.gz`, `.tgz`, `.gz`, `.bz2`, `.7z`, `.rar`) is committed. Archives are
  build outputs — produce them via the build system.
- **`binary-compiled-artifact`** (error): A compiled artifact (`.so`, `.dll`,
  `.dylib`, `.exe`, `.o`, `.a`, `.class`, `.jar`, `.wasm`) is committed.
  Compiled artifacts are build outputs.

## File Types Covered

- **Images**: `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.tiff`, `.webp`,
  `.ico`, `.psd`, `.svg`
- **Video**: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`
- **Audio**: `.mp3`, `.wav`, `.flac`, `.aac`, `.ogg`
- **Documents**: `.pdf`, `.doc`, `.docx`, `.xls`, `.xlsx`, `.ppt`, `.pptx`
- **Archives**: `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.gz`, `.bz2`, `.7z`, `.rar`
- **Compiled**: `.so`, `.dll`, `.dylib`, `.exe`, `.o`, `.a`, `.class`, `.jar`,
  `.wasm`
- **Fonts**: `.ttf`, `.otf`, `.woff`, `.woff2`

## Examples

✅ Good: `assets/logo.png` (small, in an asset directory)
✅ Good: `public/hero.webp` (in a public static dir)
❌ Bad: `src/logo.png` (binary in a source directory)
❌ Bad: `release/build.zip` (committed archive)
❌ Bad: `lib/libfoo.so` (committed compiled artifact)
❌ Bad: `assets/big.mp4` at 12 MB (oversized — use an external store)
