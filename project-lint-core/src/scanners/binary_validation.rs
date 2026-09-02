//! Binary validation scanner — detects binary files committed to a repository
//! that should be tracked via Git LFS, are oversized, live in source
//! directories, duplicate other binaries, or are build artifacts (archives /
//! compiled blobs) that should never be checked in.
//!
//! The scanner walks the project tree (honoring the centralized exclusion
//! list) and inspects files by extension. For each matching file it records
//! the size and (for duplicate detection) a SHA-256 digest of the content.
//!
//! Rules emitted:
//!
//! - **binary-lfs-required** — file exceeds the LFS threshold (default 1 MB).
//! - **binary-oversized** — file exceeds the hard size limit (default 10 MB).
//! - **binary-in-source-dir** — binary located inside a source directory.
//! - **binary-duplicate** — two or more binaries share identical content.
//! - **binary-archive-committed** — a build archive is committed.
//! - **binary-compiled-artifact** — a compiled artifact is committed.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

/// Default LFS threshold: 1 MB. Files larger than this should use Git LFS.
pub const DEFAULT_LFS_THRESHOLD_BYTES: u64 = 1_048_576;

/// Default hard size limit: 10 MB. Files larger than this are errors.
pub const DEFAULT_MAX_SIZE_BYTES: u64 = 10_485_760;

/// Default binary file extensions scanned by the scanner.
pub const DEFAULT_BINARY_EXTENSIONS: &[&str] = &[
    // Images
    "png", "jpg", "jpeg", "gif", "bmp", "tiff", "tif", "webp", "ico", "psd", "svg",
    // Video
    "mp4", "mov", "avi", "mkv", "webm", "flv", // Audio
    "mp3", "wav", "flac", "aac", "ogg", // Documents
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", // Archives
    "zip", "tar", "tgz", "gz", "bz2", "7z", "rar", // Compiled artifacts
    "so", "dll", "dylib", "exe", "o", "a", "class", "jar", "wasm", // Fonts
    "ttf", "otf", "woff", "woff2",
];

/// Archive extensions (subset of the binary set) that trigger
/// `binary-archive-committed`.
pub const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "tar", "tgz", "gz", "bz2", "7z", "rar"];

/// Compiled-artifact extensions that trigger `binary-compiled-artifact`.
pub const COMPILED_EXTENSIONS: &[&str] = &[
    "so", "dll", "dylib", "exe", "o", "a", "class", "jar", "wasm",
];

/// Default directory names treated as source directories for the
/// `binary-in-source-dir` rule. A binary whose path contains any of these as
/// a path segment is flagged.
pub const DEFAULT_SOURCE_DIRS: &[&str] = &["src", "lib", "cmd", "internal", "app", "core", "bin"];

/// Directory names where binaries are expected and therefore exempt from the
/// `binary-in-source-dir` rule even when nested under a source dir.
pub const ASSET_DIRS: &[&str] = &[
    "assets",
    "public",
    "static",
    "resources",
    "media",
    "img",
    "images",
    "fonts",
];

pub struct BinaryValidationScanner {
    lfs_threshold_bytes: u64,
    max_size_bytes: u64,
    check_lfs: bool,
    check_oversized: bool,
    check_source_dir: bool,
    check_duplicate: bool,
    check_archive: bool,
    check_compiled: bool,
    binary_extensions: Vec<String>,
    source_dirs: Vec<String>,
    excluded: Vec<String>,
}

impl BinaryValidationScanner {
    pub fn new() -> Self {
        Self {
            lfs_threshold_bytes: DEFAULT_LFS_THRESHOLD_BYTES,
            max_size_bytes: DEFAULT_MAX_SIZE_BYTES,
            check_lfs: true,
            check_oversized: true,
            check_source_dir: true,
            check_duplicate: true,
            check_archive: true,
            check_compiled: true,
            binary_extensions: DEFAULT_BINARY_EXTENSIONS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            source_dirs: DEFAULT_SOURCE_DIRS.iter().map(|s| s.to_string()).collect(),
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        lfs_threshold_bytes: u64,
        max_size_bytes: u64,
        check_lfs: bool,
        check_oversized: bool,
        check_source_dir: bool,
        check_duplicate: bool,
        check_archive: bool,
        check_compiled: bool,
        extra_binary_extensions: Vec<String>,
        extra_source_dirs: Vec<String>,
    ) -> Self {
        let mut binary_extensions: Vec<String> = DEFAULT_BINARY_EXTENSIONS
            .iter()
            .map(|s| s.to_string())
            .collect();
        for ex in &extra_binary_extensions {
            let lower = ex.to_lowercase();
            if !binary_extensions.iter().any(|e| e == &lower) {
                binary_extensions.push(lower);
            }
        }
        let mut source_dirs: Vec<String> =
            DEFAULT_SOURCE_DIRS.iter().map(|s| s.to_string()).collect();
        for d in &extra_source_dirs {
            let lower = d.to_lowercase();
            if !source_dirs.iter().any(|e| e == &lower) {
                source_dirs.push(lower);
            }
        }
        Self {
            lfs_threshold_bytes,
            max_size_bytes,
            check_lfs,
            check_oversized,
            check_source_dir,
            check_duplicate,
            check_archive,
            check_compiled,
            binary_extensions,
            source_dirs,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        lfs_threshold_bytes: u64,
        max_size_bytes: u64,
        check_lfs: bool,
        check_oversized: bool,
        check_source_dir: bool,
        check_duplicate: bool,
        check_archive: bool,
        check_compiled: bool,
        extra_binary_extensions: Vec<String>,
        extra_source_dirs: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        let mut s = Self::with_config(
            lfs_threshold_bytes,
            max_size_bytes,
            check_lfs,
            check_oversized,
            check_source_dir,
            check_duplicate,
            check_archive,
            check_compiled,
            extra_binary_extensions,
            extra_source_dirs,
        );
        s.excluded = excluded;
        s
    }

    /// Scan a project root for binary-file violations.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        // Collect binary file metadata for the per-file rules and the
        // duplicate-detection pass.
        let mut binaries: Vec<BinaryEntry> = Vec::new();

        for entry in walk_project(root, &self.excluded, 8).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.to_string_lossy().to_string();
            if is_excluded_rel(&rel_str, &self.excluded) {
                continue;
            }
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_lowercase(),
                None => continue,
            };
            // Handle compound extension .tar.gz — the WalkDir extension()
            // returns "gz"; treat files ending in ".tar.gz" as archive.
            let lower_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let is_tar_gz = lower_name.ends_with(".tar.gz");
            if !self.is_binary_ext(&ext) && !is_tar_gz {
                continue;
            }

            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    debug!("binary_validation: cannot stat {:?}: {}", path, e);
                    continue;
                }
            };
            let size = metadata.len();

            // Per-file rules.
            self.check_file_rules(&rel_str, &ext, is_tar_gz, size, &mut issues);

            // Collect for duplicate detection (only when enabled).
            if self.check_duplicate {
                let digest = if size > 0 {
                    match compute_sha256(path) {
                        Ok(d) => Some(d),
                        Err(e) => {
                            debug!("binary_validation: cannot hash {:?}: {}", path, e);
                            None
                        }
                    }
                } else {
                    // Empty files share a conceptual "empty" digest; use a
                    // fixed sentinel so they group together without I/O.
                    Some(empty_digest())
                };
                if let Some(digest) = digest {
                    binaries.push(BinaryEntry {
                        rel: rel_str,
                        size,
                        digest,
                    });
                }
            }
        }

        if self.check_duplicate {
            self.check_duplicates(&binaries, &mut issues);
        }

        Ok(issues)
    }

    fn is_binary_ext(&self, ext: &str) -> bool {
        self.binary_extensions.iter().any(|e| e == ext)
    }

    fn check_file_rules(
        &self,
        rel: &str,
        ext: &str,
        is_tar_gz: bool,
        size: u64,
        issues: &mut Vec<ScannerIssue>,
    ) {
        // binary-compiled-artifact (highest priority — errors).
        if self.check_compiled && is_compiled_ext(ext) {
            issues.push(ScannerIssue::new(
                "binary-compiled-artifact",
                "error",
                rel,
                format!(
                    "compiled artifact '*.{}' committed to the repository — build outputs should not be tracked in git",
                    ext
                ),
            ));
        }

        // binary-archive-committed.
        if self.check_archive && (is_archive_ext(ext) || is_tar_gz) {
            let label = if is_tar_gz { "tar.gz" } else { ext };
            issues.push(ScannerIssue::new(
                "binary-archive-committed",
                "warning",
                rel,
                format!(
                    "archive '*.{}' committed to the repository — archives are build outputs, produce them via the build system",
                    label
                ),
            ));
        }

        // binary-oversized (error).
        if self.check_oversized && size > self.max_size_bytes {
            issues.push(ScannerIssue::new(
                "binary-oversized",
                "error",
                rel,
                format!(
                    "binary file is {} bytes (limit {} bytes) — use an external asset store",
                    size, self.max_size_bytes,
                ),
            ));
        }

        // binary-lfs-required (warning).
        if self.check_lfs && size > self.lfs_threshold_bytes {
            issues.push(ScannerIssue::new(
                "binary-lfs-required",
                "warning",
                rel,
                format!(
                    "binary file is {} bytes (threshold {} bytes) — track via Git LFS instead of committing directly",
                    size, self.lfs_threshold_bytes,
                ),
            ));
        }

        // binary-in-source-dir (warning).
        if self.check_source_dir && self.is_in_source_dir(rel) {
            issues.push(ScannerIssue::new(
                "binary-in-source-dir",
                "warning",
                rel,
                "binary file located in a source directory — move to an asset directory (assets/, public/, static/, resources/)",
            ));
        }
    }

    /// Returns true when `rel` contains a source-dir segment but is not itself
    /// inside an asset directory. This catches `src/foo.png` and
    /// `packages/bar/src/logo.png` while allowing `src/assets/icon.png`.
    fn is_in_source_dir(&self, rel: &str) -> bool {
        let segments: Vec<&str> = rel.split('/').collect();
        let mut in_source = false;
        for seg in &segments {
            let lower = seg.to_lowercase();
            if ASSET_DIRS.contains(&lower.as_str()) {
                // Once inside an asset dir, the file is expected there.
                return false;
            }
            if self.source_dirs.iter().any(|d| d == &lower) {
                in_source = true;
            }
        }
        in_source
    }

    fn check_duplicates(&self, binaries: &[BinaryEntry], issues: &mut Vec<ScannerIssue>) {
        // Group by digest; any group with >1 entry is a duplicate set.
        let mut by_digest: HashMap<&[u8], Vec<&BinaryEntry>> = HashMap::new();
        for b in binaries {
            by_digest.entry(&b.digest).or_default().push(b);
        }
        // Sort each group by path for deterministic output.
        for group in by_digest.values() {
            if group.len() < 2 {
                continue;
            }
            let mut sorted: Vec<&&BinaryEntry> = group.iter().collect();
            sorted.sort_by(|a, b| a.rel.cmp(&b.rel));
            let paths: Vec<&str> = sorted.iter().map(|e| e.rel.as_str()).collect();
            for entry in &sorted {
                issues.push(ScannerIssue::new(
                    "binary-duplicate",
                    "warning",
                    &entry.rel,
                    format!(
                        "duplicate binary (identical content) also at: {}",
                        paths
                            .iter()
                            .filter(|p| **p != entry.rel)
                            .copied()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
    }
}

impl Default for BinaryValidationScanner {
    fn default() -> Self {
        Self::new()
    }
}

struct BinaryEntry {
    rel: String,
    #[allow(dead_code)]
    size: u64,
    digest: Vec<u8>,
}

fn is_archive_ext(ext: &str) -> bool {
    ARCHIVE_EXTENSIONS.contains(&ext)
}

fn is_compiled_ext(ext: &str) -> bool {
    COMPILED_EXTENSIONS.contains(&ext)
}

fn compute_sha256(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_vec())
}

fn empty_digest() -> Vec<u8> {
    let hasher = Sha256::new();
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_bytes(dir: &Path, rel: &str, bytes: &[u8]) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, bytes).unwrap();
    }

    fn one_mb() -> Vec<u8> {
        vec![0u8; 1_048_577]
    }

    // -----------------------------------------------------------------------
    // binary-lfs-required
    // -----------------------------------------------------------------------

    #[test]
    fn flags_file_over_lfs_threshold() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "assets/big.png", &one_mb());
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            true,
            false,
            false,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "binary-lfs-required"));
        Ok(())
    }

    #[test]
    fn small_binary_does_not_trigger_lfs() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "assets/small.png", b"PNG-data");
        let scanner = BinaryValidationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "binary-lfs-required"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // binary-oversized
    // -----------------------------------------------------------------------

    #[test]
    fn flags_oversized_binary_as_error() -> Result<()> {
        let dir = TempDir::new()?;
        // 11 MB — over the 10 MB default limit.
        write_bytes(&dir.path(), "assets/huge.mp4", &vec![0u8; 11 * 1_048_576]);
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            true,
            false,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let oversized = issues
            .iter()
            .find(|i| i.rule == "binary-oversized")
            .expect("expected oversized issue");
        assert_eq!(oversized.severity, "error");
        Ok(())
    }

    #[test]
    fn binary_under_max_size_not_oversized() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "assets/ok.mp4", &vec![0u8; 5 * 1_048_576]);
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            true,
            false,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "binary-oversized"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // binary-in-source-dir
    // -----------------------------------------------------------------------

    #[test]
    fn flags_binary_in_source_dir() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "src/logo.png", b"PNG");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            true,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "binary-in-source-dir"));
        Ok(())
    }

    #[test]
    fn binary_in_asset_dir_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "src/assets/icon.png", b"PNG");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            true,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "binary-in-source-dir"));
        Ok(())
    }

    #[test]
    fn binary_in_public_dir_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "public/hero.png", b"PNG");
        let scanner = BinaryValidationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "binary-in-source-dir"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // binary-duplicate
    // -----------------------------------------------------------------------

    #[test]
    fn flags_duplicate_binaries() -> Result<()> {
        let dir = TempDir::new()?;
        let content = b"duplicate-png-content";
        write_bytes(&dir.path(), "assets/a/logo.png", content);
        write_bytes(&dir.path(), "assets/b/logo.png", content);
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            false,
            true,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let dups: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == "binary-duplicate")
            .collect();
        assert_eq!(dups.len(), 2, "expected one issue per duplicate file");
        Ok(())
    }

    #[test]
    fn distinct_binaries_not_flagged_as_duplicate() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "assets/a/logo.png", b"content-a");
        write_bytes(&dir.path(), "assets/b/logo.png", b"content-b");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            false,
            true,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "binary-duplicate"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // binary-archive-committed
    // -----------------------------------------------------------------------

    #[test]
    fn flags_committed_archive() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "release/build.zip", b"ZIP");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            false,
            false,
            true,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "binary-archive-committed"));
        Ok(())
    }

    #[test]
    fn flags_tar_gz_archive() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "release.tar.gz", b"GZ");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            false,
            false,
            true,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let arc = issues
            .iter()
            .find(|i| i.rule == "binary-archive-committed")
            .expect("expected archive issue");
        assert!(arc.message.contains("tar.gz"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // binary-compiled-artifact
    // -----------------------------------------------------------------------

    #[test]
    fn flags_compiled_artifact_as_error() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "lib/libfoo.so", b"ELF");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            false,
            false,
            false,
            true,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let compiled = issues
            .iter()
            .find(|i| i.rule == "binary-compiled-artifact")
            .expect("expected compiled-artifact issue");
        assert_eq!(compiled.severity, "error");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn clean_repo_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        let scanner = BinaryValidationScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn config_can_disable_all_checks() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "src/big.zip", &one_mb());
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            false,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn excluded_dirs_are_skipped() -> Result<()> {
        let dir = TempDir::new()?;
        // A binary inside target/ (excluded by default) must not be flagged.
        write_bytes(&dir.path(), "target/debug/app.png", &one_mb());
        let scanner = BinaryValidationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.is_empty(),
            "excluded dir should be skipped: {:?}",
            issues
        );
        Ok(())
    }

    #[test]
    fn extra_binary_extension_is_scanned() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "assets/data.dat", &one_mb());
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            true,
            false,
            false,
            false,
            false,
            false,
            vec!["dat".to_string()],
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "binary-lfs-required"));
        Ok(())
    }

    #[test]
    fn extra_source_dir_is_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "customsrc/icon.png", b"PNG");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            true,
            false,
            false,
            false,
            Vec::new(),
            vec!["customsrc".to_string()],
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "binary-in-source-dir"));
        Ok(())
    }

    #[test]
    fn svg_is_treated_as_binary() -> Result<()> {
        let dir = TempDir::new()?;
        write_bytes(&dir.path(), "src/diagram.svg", b"<svg/>");
        let scanner = BinaryValidationScanner::with_config(
            DEFAULT_LFS_THRESHOLD_BYTES,
            DEFAULT_MAX_SIZE_BYTES,
            false,
            false,
            true,
            false,
            false,
            false,
            Vec::new(),
            Vec::new(),
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "binary-in-source-dir"));
        Ok(())
    }

    #[test]
    fn default_thresholds_match_prd() {
        assert_eq!(DEFAULT_LFS_THRESHOLD_BYTES, 1_048_576);
        assert_eq!(DEFAULT_MAX_SIZE_BYTES, 10_485_760);
    }
}
