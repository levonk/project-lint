//! Go config scanner — validates `go.mod` and `go.sum` for Go module
//! conventions: Go version specification, no local `replace` directives, no
//! `// indirect` deps in `go.mod`, and `go.sum` presence when `go.mod` exists.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct GoConfigScanner {
    require_go_sum: bool,
    forbid_local_replace: bool,
    excluded: Vec<String>,
}

impl GoConfigScanner {
    pub fn new() -> Self {
        Self {
            require_go_sum: true,
            forbid_local_replace: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_go_sum: bool, forbid_local_replace: bool) -> Self {
        Self {
            require_go_sum,
            forbid_local_replace,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_go_sum: bool,
        forbid_local_replace: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_go_sum,
            forbid_local_replace,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();
        let mut go_mod_paths: Vec<(std::path::PathBuf, String)> = Vec::new();
        let mut has_go_sum = false;

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "go.mod" {
                go_mod_paths.push((path.to_path_buf(), rel));
            }
            if name == "go.sum" {
                has_go_sum = true;
            }
        }

        for (path, rel) in &go_mod_paths {
            issues.extend(self.scan_go_mod(path, rel));
        }

        if self.require_go_sum && !go_mod_paths.is_empty() && !has_go_sum {
            issues.push(ScannerIssue::new(
                "go-sum-present",
                "error",
                "go.sum",
                "go.mod exists but go.sum is missing",
            ));
        }

        Ok(issues)
    }

    fn scan_go_mod(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut has_go_version = false;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("go ") {
                has_go_version = true;
                let version = rest.trim();
                if let Some(minor) = parse_go_minor(version) {
                    if minor < 22 {
                        issues.push(
                            ScannerIssue::new(
                                "go-mod-go-version",
                                "warning",
                                rel,
                                format!("Go version '{}' is older than 1.22", version),
                            )
                            .at_line(i + 1),
                        );
                    }
                }
            }

            if self.forbid_local_replace && trimmed.starts_with("replace ") {
                if trimmed.contains("=> ../") || trimmed.contains("=> ./") {
                    issues.push(
                        ScannerIssue::new(
                            "go-mod-no-replace-local",
                            "error",
                            rel,
                            "local filesystem replace directive should not be committed",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if trimmed.starts_with("require ") && trimmed.contains("// indirect") {
                issues.push(
                    ScannerIssue::new(
                        "go-mod-no-indirect-in-mod",
                        "info",
                        rel,
                        "// indirect dependencies belong in go.sum, not go.mod require block",
                    )
                    .at_line(i + 1),
                );
            }
        }

        if !has_go_version {
            issues.push(ScannerIssue::new(
                "go-mod-go-version",
                "warning",
                rel,
                "go.mod missing 'go' version directive",
            ));
        }

        issues
    }
}

fn parse_go_minor(version: &str) -> Option<u32> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        parts[1].parse::<u32>().ok()
    } else {
        None
    }
}

impl Default for GoConfigScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn valid_go_mod_with_go_sum_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/foo\n\ngo 1.22\n\nrequire (\n\tbar v1.0.0\n)\n",
        )?;
        std::fs::write(
            dir.path().join("go.sum"),
            "example.com/bar v1.0.0 h1:abc=\n",
        )?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn old_go_version_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/foo\n\ngo 1.18\n",
        )?;
        std::fs::write(dir.path().join("go.sum"), "")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "go-mod-go-version"));
        Ok(())
    }

    #[test]
    fn missing_go_version_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("go.mod"), "module example.com/foo\n")?;
        std::fs::write(dir.path().join("go.sum"), "")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "go-mod-go-version"));
        Ok(())
    }

    #[test]
    fn local_replace_flags_error() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/foo\n\ngo 1.22\n\nreplace example.com/bar => ../bar\n",
        )?;
        std::fs::write(dir.path().join("go.sum"), "")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "go-mod-no-replace-local" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn remote_replace_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/foo\n\ngo 1.22\n\nreplace example.com/bar => example.com/bar/v2 v2.0.0\n",
        )?;
        std::fs::write(dir.path().join("go.sum"), "")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "go-mod-no-replace-local"));
        Ok(())
    }

    #[test]
    fn indirect_in_require_flags_info() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/foo\n\ngo 1.22\n\nrequire example.com/bar v1.0.0 // indirect\n",
        )?;
        std::fs::write(dir.path().join("go.sum"), "")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "go-mod-no-indirect-in-mod" && i.severity == "info"));
        Ok(())
    }

    #[test]
    fn missing_go_sum_flags_error() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/foo\n\ngo 1.22\n",
        )?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "go-sum-present" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn no_go_mod_is_silent() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_go_mod_flags_missing_version() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("go.mod"), "")?;
        std::fs::write(dir.path().join("go.sum"), "")?;
        let scanner = GoConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "go-mod-go-version"));
        Ok(())
    }
}
