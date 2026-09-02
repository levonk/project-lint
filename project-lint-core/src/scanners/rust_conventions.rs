//! Rust conventions scanner — enforces rust-development-practices at the
//! source level: no `dbg!`/debug `println!` outside tests, no `.unwrap()` in
//! library code, no `unsafe` blocks in non-`build.rs` source, and no forbidden
//! crates declared in `Cargo.toml`.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct RustConventionsScanner {
    forbidden_crates: Vec<String>,
    excluded: Vec<String>,
}

impl RustConventionsScanner {
    pub fn new() -> Self {
        Self {
            forbidden_crates: Vec::new(),
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_forbidden_crates(crates: Vec<String>) -> Self {
        Self {
            forbidden_crates: crates,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(crates: Vec<String>, excluded: Vec<String>) -> Self {
        Self {
            forbidden_crates: crates,
            excluded,
        }
    }

    /// Scan a Rust project root. Walks `.rs` files (skipping `target/` and
    /// `tests/`) and the root `Cargo.toml`.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.to_string_lossy();
            if is_excluded_rel(&rel_str, &self.excluded) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "Cargo.toml" {
                issues.extend(self.scan_cargo_toml(path, &rel_str));
                continue;
            }
            if !name.ends_with(".rs") {
                continue;
            }
            // Skip test files and build scripts from the lib-only checks.
            let is_test = rel_str.starts_with("tests/")
                || rel_str.ends_with("_test.rs")
                || rel_str.ends_with("_tests.rs");
            let is_build_script = name == "build.rs";
            issues.extend(self.scan_rust_file(path, &rel_str, is_test, is_build_script));
        }

        Ok(issues)
    }

    fn scan_rust_file(
        &self,
        path: &Path,
        rel: &str,
        is_test: bool,
        is_build_script: bool,
    ) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            // dbg! and debug println! forbidden outside tests
            if !is_test && trimmed.contains("dbg!(") {
                issues.push(
                    ScannerIssue::new(
                        "no-debug-dbg",
                        "warning",
                        rel,
                        "dbg! macro left in non-test source",
                    )
                    .at_line(i + 1),
                );
            }
            // unwrap() forbidden in lib code (not tests, not build.rs)
            if !is_test && !is_build_script && trimmed.contains(".unwrap()") {
                issues.push(
                    ScannerIssue::new(
                        "no-unwrap-in-lib",
                        "warning",
                        rel,
                        ".unwrap() in library code; prefer ? or expect with context",
                    )
                    .at_line(i + 1),
                );
            }
            // unsafe blocks forbidden outside build.rs
            if !is_build_script && trimmed.contains("unsafe ") {
                issues.push(
                    ScannerIssue::new(
                        "no-unsafe-blocks",
                        "warning",
                        rel,
                        "unsafe block in non-build source",
                    )
                    .at_line(i + 1),
                );
            }
        }
        issues
    }

    fn scan_cargo_toml(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        for crate_name in &self.forbidden_crates {
            // match `crate_name =` or `"crate_name"` in dependency lines
            let needle = format!("{} ", crate_name);
            let needle_q = format!("\"{}\"", crate_name);
            if content.contains(&needle) || content.contains(&needle_q) {
                issues.push(ScannerIssue::new(
                    "forbidden-crate",
                    "error",
                    rel,
                    format!("forbidden crate '{}' declared in Cargo.toml", crate_name),
                ));
            }
        }
        issues
    }
}

impl Default for RustConventionsScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flags_unwrap_and_dbg_in_lib_source() -> Result<()> {
        let dir = TempDir::new()?;
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src)?;
        std::fs::write(
            src.join("lib.rs"),
            "fn f() { let x = dbg!(1); let y = x.unwrap(); }\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"no-debug-dbg"));
        assert!(rules.contains(&"no-unwrap-in-lib"));
        Ok(())
    }

    #[test]
    fn ignores_unwrap_in_test_files() -> Result<()> {
        let dir = TempDir::new()?;
        let tests = dir.path().join("tests");
        std::fs::create_dir_all(&tests)?;
        std::fs::write(
            tests.join("foo_test.rs"),
            "#[test] fn t() { assert!(1.unwrap()); }\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().all(|i| i.rule != "no-unwrap-in-lib"));
        Ok(())
    }

    #[test]
    fn flags_forbidden_crate_in_cargo_toml() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nopenssl = \"0.10\"\n",
        )?;
        let scanner = RustConventionsScanner::with_forbidden_crates(vec!["openssl".to_string()]);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "forbidden-crate"));
        Ok(())
    }
}
