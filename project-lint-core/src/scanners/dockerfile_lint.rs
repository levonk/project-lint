//! Dockerfile lint scanner — enforces container best practices: pinned image
//! digests, no `COPY .`, and a non-root `USER` declaration.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;
use walkdir::WalkDir;

pub struct DockerfileLintScanner {
    require_pinned_digests: bool,
    require_non_root_user: bool,
    forbid_copy_dot: bool,
}

impl DockerfileLintScanner {
    pub fn new() -> Self {
        Self {
            require_pinned_digests: true,
            require_non_root_user: true,
            forbid_copy_dot: true,
        }
    }

    pub fn with_config(
        require_pinned_digests: bool,
        require_non_root_user: bool,
        forbid_copy_dot: bool,
    ) -> Self {
        Self {
            require_pinned_digests,
            require_non_root_user,
            forbid_copy_dot,
        }
    }

    /// Scan a project for Dockerfiles and lint each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in WalkDir::new(root)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with("Dockerfile") && !name.ends_with(".dockerfile") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            issues.extend(self.scan_dockerfile(path, &rel));
        }

        Ok(issues)
    }

    fn scan_dockerfile(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut has_user = false;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("FROM ") {
                if self.require_pinned_digests && !rest.contains("@sha256:") {
                    issues.push(
                        ScannerIssue::new(
                            "pin-image-digests",
                            "warning",
                            rel,
                            format!(
                                "FROM '{}' not pinned by digest",
                                rest.split_whitespace().next().unwrap_or(rest)
                            ),
                        )
                        .at_line(i + 1),
                    );
                }
            }
            if self.forbid_copy_dot && trimmed.starts_with("COPY ") {
                // match "COPY . " or "COPY ./" at the start of args
                let args = trimmed.strip_prefix("COPY ").unwrap_or("");
                if args.starts_with('.') {
                    issues.push(
                        ScannerIssue::new(
                            "no-copy-dot",
                            "warning",
                            rel,
                            "COPY with '.' build context; copy only required paths",
                        )
                        .at_line(i + 1),
                    );
                }
            }
            if trimmed.starts_with("USER ") {
                has_user = true;
            }
        }

        if self.require_non_root_user && !has_user {
            issues.push(ScannerIssue::new(
                "require-non-root-user",
                "warning",
                rel,
                "Dockerfile missing non-root USER declaration",
            ));
        }

        issues
    }
}

impl Default for DockerfileLintScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn flags_unpinned_from_copy_dot_and_missing_user() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:20\nCOPY . /app\nRUN npm install\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"pin-image-digests"));
        assert!(rules.contains(&"no-copy-dot"));
        assert!(rules.contains(&"require-non-root-user"));
        Ok(())
    }

    #[test]
    fn clean_dockerfile_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:20@sha256:abc\nCOPY package.json /app\nUSER node\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn config_can_disable_checks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("Dockerfile"), "FROM node:20\nCOPY . /app\n")?;
        let scanner = DockerfileLintScanner::with_config(false, false, false);
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }
}
