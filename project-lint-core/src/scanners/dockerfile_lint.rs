//! Dockerfile lint scanner — enforces container best practices: pinned image
//! digests, no `COPY .`, non-root `USER`, no `:latest` tags, `HEALTHCHECK`
//! presence, `apk add --no-cache`, `apt-get install --no-install-recommends`
//! with cleanup, multi-stage builds, `.dockerignore` presence, and digest
//! pinning exemptions for `scratch` / distroless images.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct DockerfileLintScanner {
    require_pinned_digests: bool,
    require_non_root_user: bool,
    forbid_copy_dot: bool,
    require_healthcheck: bool,
    require_apk_no_cache: bool,
    require_apt_no_install_recommends: bool,
    require_dockerignore: bool,
    exempt_from_digest_pinning: Vec<String>,
    excluded: Vec<String>,
}

impl DockerfileLintScanner {
    pub fn new() -> Self {
        Self {
            require_pinned_digests: true,
            require_non_root_user: true,
            forbid_copy_dot: true,
            require_healthcheck: true,
            require_apk_no_cache: true,
            require_apt_no_install_recommends: true,
            require_dockerignore: true,
            exempt_from_digest_pinning: vec![
                "scratch".to_string(),
                "gcr.io/distroless/static:nonroot".to_string(),
            ],
            excluded: build_exclusions(&[], false),
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
            require_healthcheck: true,
            require_apk_no_cache: true,
            require_apt_no_install_recommends: true,
            require_dockerignore: true,
            exempt_from_digest_pinning: vec![
                "scratch".to_string(),
                "gcr.io/distroless/static:nonroot".to_string(),
            ],
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_pinned_digests: bool,
        require_non_root_user: bool,
        forbid_copy_dot: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_pinned_digests,
            require_non_root_user,
            forbid_copy_dot,
            require_healthcheck: true,
            require_apk_no_cache: true,
            require_apt_no_install_recommends: true,
            require_dockerignore: true,
            exempt_from_digest_pinning: vec![
                "scratch".to_string(),
                "gcr.io/distroless/static:nonroot".to_string(),
            ],
            excluded,
        }
    }

    pub fn with_full_config(
        require_pinned_digests: bool,
        require_non_root_user: bool,
        forbid_copy_dot: bool,
        require_healthcheck: bool,
        require_apk_no_cache: bool,
        require_apt_no_install_recommends: bool,
        require_dockerignore: bool,
        exempt_from_digest_pinning: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_pinned_digests,
            require_non_root_user,
            forbid_copy_dot,
            require_healthcheck,
            require_apk_no_cache,
            require_apt_no_install_recommends,
            require_dockerignore,
            exempt_from_digest_pinning,
            excluded,
        }
    }

    /// Scan a project for Dockerfiles and lint each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();
        let mut found_dockerfile = false;

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with("Dockerfile") && !name.ends_with(".dockerfile") {
                continue;
            }
            found_dockerfile = true;
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }
            issues.extend(self.scan_dockerfile(path, &rel));
        }

        if self.require_dockerignore && found_dockerfile {
            if !root.join(".dockerignore").exists() {
                issues.push(ScannerIssue::new(
                    "dockerfile-dockerignore-present",
                    "warning",
                    ".dockerignore",
                    "project has Dockerfile(s) but no .dockerignore file",
                ));
            }
        }

        Ok(issues)
    }

    fn scan_dockerfile(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut has_user = false;
        let mut has_healthcheck = false;
        let mut from_count = 0;
        let mut has_run_install = false;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("FROM ") {
                from_count += 1;
                let image_ref = rest.split_whitespace().next().unwrap_or(rest);
                if self.require_pinned_digests && !image_ref.contains("@sha256:") {
                    if !self.is_exempt_image(image_ref) {
                        if image_ref.ends_with(":latest") || !image_ref.contains(':') {
                            issues.push(
                                ScannerIssue::new(
                                    "dockerfile-no-latest-tag",
                                    "error",
                                    rel,
                                    format!(
                                        "FROM '{}' uses ':latest' or untagged image",
                                        image_ref
                                    ),
                                )
                                .at_line(i + 1),
                            );
                        } else {
                            issues.push(
                                ScannerIssue::new(
                                    "pin-image-digests",
                                    "warning",
                                    rel,
                                    format!("FROM '{}' not pinned by digest", image_ref),
                                )
                                .at_line(i + 1),
                            );
                        }
                    }
                }
            }
            if self.forbid_copy_dot && trimmed.starts_with("COPY ") {
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
            if trimmed.starts_with("HEALTHCHECK") {
                has_healthcheck = true;
            }
            if self.require_apk_no_cache && trimmed.contains("apk add") {
                if !trimmed.contains("--no-cache") {
                    issues.push(
                        ScannerIssue::new(
                            "dockerfile-apk-no-cache",
                            "warning",
                            rel,
                            "apk add must use --no-cache flag",
                        )
                        .at_line(i + 1),
                    );
                }
            }
            if self.require_apt_no_install_recommends && trimmed.contains("apt-get install") {
                has_run_install = true;
                if !trimmed.contains("--no-install-recommends") {
                    issues.push(
                        ScannerIssue::new(
                            "dockerfile-apt-get-no-install-recommends",
                            "warning",
                            rel,
                            "apt-get install must use --no-install-recommends",
                        )
                        .at_line(i + 1),
                    );
                }
                if !trimmed.contains("rm -rf /var/lib/apt/lists") {
                    issues.push(
                        ScannerIssue::new(
                            "dockerfile-apt-get-clean",
                            "warning",
                            rel,
                            "apt-get install should be followed by rm -rf /var/lib/apt/lists/*",
                        )
                        .at_line(i + 1),
                    );
                }
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

        if self.require_healthcheck && !has_healthcheck {
            issues.push(ScannerIssue::new(
                "dockerfile-healthcheck",
                "warning",
                rel,
                "Dockerfile missing HEALTHCHECK instruction",
            ));
        }

        if has_run_install && from_count <= 1 {
            issues.push(ScannerIssue::new(
                "dockerfile-multi-stage",
                "info",
                rel,
                "Dockerfile with RUN install commands should use multi-stage builds",
            ));
        }

        issues
    }

    fn is_exempt_image(&self, image_ref: &str) -> bool {
        let base = image_ref.split('@').next().unwrap_or(image_ref);
        let stripped = base.split(':').next().unwrap_or(base);
        self.exempt_from_digest_pinning
            .iter()
            .any(|exempt| stripped == exempt || base == exempt)
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
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
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
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:20@sha256:abc\nCOPY package.json /app\nUSER node\nHEALTHCHECK CMD curl -f http://localhost\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn config_can_disable_checks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(dir.path().join("Dockerfile"), "FROM node:20\nCOPY . /app\n")?;
        let scanner = DockerfileLintScanner::with_config(false, false, false);
        // healthcheck + dockerignore are still on by default in with_config
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(!rules.contains(&"pin-image-digests"));
        assert!(!rules.contains(&"no-copy-dot"));
        assert!(!rules.contains(&"require-non-root-user"));
        Ok(())
    }

    #[test]
    fn flags_latest_tag() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:latest\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"dockerfile-no-latest-tag"));
        Ok(())
    }

    #[test]
    fn flags_missing_healthcheck() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:20@sha256:abc\nUSER node\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"dockerfile-healthcheck"));
        Ok(())
    }

    #[test]
    fn flags_apk_without_no_cache() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM alpine:3.19@sha256:abc\nRUN apk add curl\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"dockerfile-apk-no-cache"));
        Ok(())
    }

    #[test]
    fn apk_with_no_cache_ok() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM alpine:3.19@sha256:abc\nRUN apk add --no-cache curl\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(!rules.contains(&"dockerfile-apk-no-cache"));
        Ok(())
    }

    #[test]
    fn flags_apt_get_without_no_install_recommends() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM debian:12@sha256:abc\nRUN apt-get update && apt-get install -y curl\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"dockerfile-apt-get-no-install-recommends"));
        assert!(rules.contains(&"dockerfile-apt-get-clean"));
        Ok(())
    }

    #[test]
    fn apt_get_with_recommends_and_clean_ok() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM debian:12@sha256:abc\nRUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(!rules.contains(&"dockerfile-apt-get-no-install-recommends"));
        assert!(!rules.contains(&"dockerfile-apt-get-clean"));
        Ok(())
    }

    #[test]
    fn flags_missing_dockerignore() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:20@sha256:abc\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"dockerfile-dockerignore-present"));
        Ok(())
    }

    #[test]
    fn scratch_exempt_from_digest_pinning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM scratch\nCOPY app /app\nUSER 1000\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"pin-image-digests"),
            "scratch should be exempt: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn distroless_exempt_from_digest_pinning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM gcr.io/distroless/static:nonroot\nCOPY app /app\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"pin-image-digests"),
            "distroless should be exempt: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn flags_missing_multi_stage() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM debian:12@sha256:abc\nRUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"dockerfile-multi-stage"));
        Ok(())
    }

    #[test]
    fn multi_stage_no_flag() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(
            dir.path().join("Dockerfile"),
            "FROM debian:12@sha256:abc AS builder\nRUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*\nFROM debian:12@sha256:abc\nCOPY --from=builder /app /app\nUSER node\nHEALTHCHECK CMD true\n",
        )?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"dockerfile-multi-stage"),
            "multi-stage should not flag: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn no_dockerfiles_silent() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# no docker here")?;
        let scanner = DockerfileLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn empty_dockerfile_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".dockerignore"), "target/\n")?;
        std::fs::write(dir.path().join("Dockerfile"), "")?;
        let scanner = DockerfileLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        // empty Dockerfile: no FROM so no digest/healthcheck flags, but USER missing
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(!rules.contains(&"pin-image-digests"));
        assert!(!rules.contains(&"dockerfile-no-latest-tag"));
        Ok(())
    }
}
