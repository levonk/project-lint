//! Makefile content scanner — validates `Makefile` content: flags Makefile
//! presence (should migrate to justfile), no absolute paths, no `cd /absolute`,
//! and just-delegation pattern when Makefile must exist.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct MakefileContentScanner {
    require_just_delegation: bool,
    excluded: Vec<String>,
}

impl MakefileContentScanner {
    pub fn new() -> Self {
        Self {
            require_just_delegation: false,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_just_delegation: bool) -> Self {
        Self {
            require_just_delegation,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(require_just_delegation: bool, excluded: Vec<String>) -> Self {
        Self {
            require_just_delegation,
            excluded,
        }
    }

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
            if name != "Makefile" && name != "makefile" && !name.ends_with(".mk") {
                continue;
            }
            issues.extend(self.scan_makefile(path, &rel_str));
        }

        Ok(issues)
    }

    fn scan_makefile(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        if rel == "Makefile" || rel == "makefile" {
            issues.push(ScannerIssue::new(
                "makefile-forbidden",
                "warning",
                rel,
                "Makefile present; should be migrated to justfile",
            ));
        }

        let has_just_delegation = content.lines().any(|l| {
            let trimmed = l.trim();
            trimmed.contains("just ") || trimmed.contains("just\t")
        });

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }

            if has_absolute_path(trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "makefile-no-absolute-paths",
                        "error",
                        rel,
                        "Makefile contains hardcoded absolute path",
                    )
                    .at_line(i + 1),
                );
            }

            if has_cd_absolute(trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "makefile-no-cd-absolute",
                        "error",
                        rel,
                        "Makefile uses 'cd /absolute/path' in a rule",
                    )
                    .at_line(i + 1),
                );
            }
        }

        if self.require_just_delegation && !has_just_delegation {
            issues.push(ScannerIssue::new(
                "makefile-uses-just-delegation",
                "info",
                rel,
                "Makefile should delegate to 'just' (e.g. 'build: ; just build')",
            ));
        }

        issues
    }
}

impl Default for MakefileContentScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn has_absolute_path(line: &str) -> bool {
    if line.contains("/Users/") || line.contains("/home/") || line.contains("/tmp/") {
        return true;
    }
    false
}

fn has_cd_absolute(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix("cd ") {
        let rest = rest.trim_start();
        if rest.starts_with('/') {
            return true;
        }
    }
    if line.contains(" cd /") {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flags_makefile_presence() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Makefile"), "all:\n\techo hello\n")?;
        let scanner = MakefileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "makefile-forbidden"));
        Ok(())
    }

    #[test]
    fn flags_absolute_paths() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Makefile"),
            "build:\n\tcp /Users/foo/bar .\n",
        )?;
        let scanner = MakefileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "makefile-no-absolute-paths"));
        Ok(())
    }

    #[test]
    fn flags_cd_absolute() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Makefile"),
            "build:\n\tcd /home/user/project && make\n",
        )?;
        let scanner = MakefileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "makefile-no-cd-absolute"));
        Ok(())
    }

    #[test]
    fn flags_missing_just_delegation_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Makefile"), "build:\n\techo hello\n")?;
        let scanner = MakefileContentScanner::with_config(true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "makefile-uses-just-delegation"));
        Ok(())
    }

    #[test]
    fn no_delegation_issue_when_just_present() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Makefile"), "build:\n\tjust build\n")?;
        let scanner = MakefileContentScanner::with_config(true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "makefile-uses-just-delegation"));
        Ok(())
    }

    #[test]
    fn silent_when_no_makefile() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = MakefileContentScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn clean_makefile_only_flags_presence() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Makefile"),
            "build:\n\techo build\ntest:\n\techo test\n",
        )?;
        let scanner = MakefileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"makefile-forbidden"));
        assert!(!rules.contains(&"makefile-no-absolute-paths"));
        assert!(!rules.contains(&"makefile-no-cd-absolute"));
        Ok(())
    }
}
