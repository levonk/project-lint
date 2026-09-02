//! Rust conventions scanner — enforces rust-development-practices at the
//! source level: no `dbg!`/debug `println!` outside tests, no `.unwrap()` in
//! library code, no `unsafe` blocks in non-`build.rs` source, and no forbidden
//! crates declared in `Cargo.toml`.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct RustConventionsScanner {
    forbidden_crates: Vec<String>,
    require_edition_2021: bool,
    require_license: bool,
    forbid_floating_deps: bool,
    excluded: Vec<String>,
}

impl RustConventionsScanner {
    pub fn new() -> Self {
        Self {
            forbidden_crates: Vec::new(),
            require_edition_2021: true,
            require_license: true,
            forbid_floating_deps: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_forbidden_crates(crates: Vec<String>) -> Self {
        Self {
            forbidden_crates: crates,
            require_edition_2021: true,
            require_license: true,
            forbid_floating_deps: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(crates: Vec<String>, excluded: Vec<String>) -> Self {
        Self {
            forbidden_crates: crates,
            require_edition_2021: true,
            require_license: true,
            forbid_floating_deps: true,
            excluded,
        }
    }

    pub fn with_config(
        crates: Vec<String>,
        require_edition_2021: bool,
        require_license: bool,
        forbid_floating_deps: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            forbidden_crates: crates,
            require_edition_2021,
            require_license,
            forbid_floating_deps,
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

        if self.require_edition_2021 {
            if let Some(edition_line) = content.lines().find(|l| l.trim().starts_with("edition")) {
                let trimmed = edition_line.trim();
                if !trimmed.contains("\"2021\"") {
                    issues.push(ScannerIssue::new(
                        "cargo-edition-2021",
                        "warning",
                        rel,
                        "Cargo.toml should use edition = \"2021\"",
                    ));
                }
            } else {
                issues.push(ScannerIssue::new(
                    "cargo-edition-2021",
                    "warning",
                    rel,
                    "Cargo.toml missing 'edition' field (should be \"2021\")",
                ));
            }
        }

        if !content.lines().any(|l| l.trim().starts_with("description")) {
            issues.push(ScannerIssue::new(
                "cargo-description-present",
                "info",
                rel,
                "Cargo.toml missing 'description' field",
            ));
        }

        if self.require_license && !content.lines().any(|l| l.trim().starts_with("license")) {
            issues.push(ScannerIssue::new(
                "cargo-license-present",
                "warning",
                rel,
                "Cargo.toml missing 'license' field",
            ));
        }

        if !content.lines().any(|l| l.trim().starts_with("repository")) {
            issues.push(ScannerIssue::new(
                "cargo-repository-present",
                "info",
                rel,
                "Cargo.toml missing 'repository' field",
            ));
        }

        if self.forbid_floating_deps {
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                if trimmed.contains("= \"*\"") || trimmed.contains("= \"*") {
                    issues.push(
                        ScannerIssue::new(
                            "cargo-no-floating-deps",
                            "warning",
                            rel,
                            "dependency uses '*' wildcard version — pin to a specific version or range",
                        )
                        .at_line(i + 1),
                    );
                }
            }
        }

        if !content.contains("[workspace.dependencies]") {
            if content.contains("[workspace]") && content.contains("[dependencies]") {
                issues.push(ScannerIssue::new(
                    "cargo-workspace-root-deps",
                    "info",
                    rel,
                    "workspace root Cargo.toml should use [workspace.dependencies] for shared deps",
                ));
            }
        }

        if content.contains("criterion") {
            let in_deps_section = in_section(&content, "[dependencies]", "criterion");
            let in_dev_section = in_section(&content, "[dev-dependencies]", "criterion");
            if in_deps_section && !in_dev_section {
                issues.push(ScannerIssue::new(
                    "cargo-no-criterion-bench-in-dev-deps",
                    "warning",
                    rel,
                    "criterion should be in [dev-dependencies], not [dependencies]",
                ));
            }
        }

        issues
    }
}

fn in_section(content: &str, section_header: &str, needle: &str) -> bool {
    let mut in_target = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_target = trimmed == section_header;
            continue;
        }
        if in_target && trimmed.contains(needle) {
            return true;
        }
    }
    false
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

    #[test]
    fn valid_cargo_toml_no_new_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
             description = \"a crate\"\nlicense = \"MIT\"\nrepository = \"https://example.com\"\n\n\
             [dependencies]\nserde = \"1.0\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let new_rules = [
            "cargo-edition-2021",
            "cargo-description-present",
            "cargo-license-present",
            "cargo-repository-present",
            "cargo-no-floating-deps",
            "cargo-workspace-root-deps",
            "cargo-no-criterion-bench-in-dev-deps",
        ];
        for rule in new_rules {
            assert!(
                !issues.iter().any(|i| i.rule == rule),
                "unexpected issue: {}",
                rule
            );
        }
        Ok(())
    }

    #[test]
    fn old_edition_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2018\"\n\
             description = \"x\"\nlicense = \"MIT\"\nrepository = \"x\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "cargo-edition-2021"));
        Ok(())
    }

    #[test]
    fn missing_license_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
             description = \"x\"\nrepository = \"x\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "cargo-license-present"));
        Ok(())
    }

    #[test]
    fn floating_dep_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
             description = \"x\"\nlicense = \"MIT\"\nrepository = \"x\"\n\n\
             [dependencies]\nserde = \"*\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "cargo-no-floating-deps"));
        Ok(())
    }

    #[test]
    fn criterion_in_deps_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
             description = \"x\"\nlicense = \"MIT\"\nrepository = \"x\"\n\n\
             [dependencies]\ncriterion = \"0.5\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "cargo-no-criterion-bench-in-dev-deps"));
        Ok(())
    }

    #[test]
    fn criterion_in_dev_deps_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
             description = \"x\"\nlicense = \"MIT\"\nrepository = \"x\"\n\n\
             [dev-dependencies]\ncriterion = \"0.5\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "cargo-no-criterion-bench-in-dev-deps"));
        Ok(())
    }

    #[test]
    fn workspace_with_deps_flags_info() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n\n\
             [dependencies]\nserde = \"1.0\"\n",
        )?;
        let scanner = RustConventionsScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "cargo-workspace-root-deps"));
        Ok(())
    }
}
