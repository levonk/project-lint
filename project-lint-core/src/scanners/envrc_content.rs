//! envrc content scanner — validates `.envrc` files for direnv-based dev
//! environments. Enforces five rules:
//!
//! 1. **No hardcoded secrets** — `export FOO=literal` patterns where the
//!    value is not a command substitution or variable reference.
//! 2. **Uses devbox or flake** — `.envrc` should contain `use devbox` or
//!    `use flake` so the dev environment is reproducible.
//! 3. **No `direnv allow`** — that is a CLI command, not a config directive.
//! 4. **watch_file devbox.json** — when using devbox, direnv should reload
//!    on devbox config changes.
//! 5. **No absolute paths** — hardcoded absolute paths are non-portable;
//!    use `$HOME`, relative paths, or `$(command -v ...)`.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct EnvrcContentScanner {
    require_devbox: bool,
    require_watch_file: bool,
    secret_patterns: Vec<String>,
    excluded: Vec<String>,
}

impl EnvrcContentScanner {
    pub fn new() -> Self {
        Self {
            require_devbox: true,
            require_watch_file: true,
            secret_patterns: default_secret_patterns(),
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_devbox: bool,
        require_watch_file: bool,
        secret_patterns: Vec<String>,
    ) -> Self {
        let secret_patterns = if secret_patterns.is_empty() {
            default_secret_patterns()
        } else {
            secret_patterns
        };
        Self {
            require_devbox,
            require_watch_file,
            secret_patterns,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_devbox: bool,
        require_watch_file: bool,
        secret_patterns: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        let secret_patterns = if secret_patterns.is_empty() {
            default_secret_patterns()
        } else {
            secret_patterns
        };
        Self {
            require_devbox,
            require_watch_file,
            secret_patterns,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != ".envrc" {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }
            issues.extend(self.scan_envrc(path, &rel));
        }

        Ok(issues)
    }

    fn scan_envrc(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut has_devbox = false;
        let mut has_flake = false;
        let mut watches_devbox = false;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            let lineno = i + 1;

            if trimmed.contains("direnv allow") {
                issues.push(
                    ScannerIssue::new(
                        "envrc-no-direnv-allow-rc",
                        "info",
                        rel,
                        "'direnv allow' is a command, not a config directive — remove it from .envrc",
                    )
                    .at_line(lineno),
                );
            }

            if let Some(issue) = self.check_secret(trimmed, rel, lineno) {
                issues.push(issue);
            }

            if let Some(issue) = check_absolute_path(trimmed, rel, lineno) {
                issues.push(issue);
            }

            if trimmed.contains("use devbox") || trimmed.contains("use_devbox") {
                has_devbox = true;
            }
            if trimmed.contains("use flake") {
                has_flake = true;
            }
            if trimmed.contains("watch_file") && trimmed.contains("devbox.json") {
                watches_devbox = true;
            }
        }

        if self.require_devbox && !has_devbox && !has_flake {
            issues.push(ScannerIssue::new(
                "envrc-uses-devbox",
                "warning",
                rel,
                ".envrc should use 'use devbox' or 'use flake' for a reproducible dev environment",
            ));
        }

        if self.require_watch_file && has_devbox && !watches_devbox {
            issues.push(ScannerIssue::new(
                "envrc-watch-file-devbox",
                "warning",
                rel,
                ".envrc uses devbox but does not 'watch_file devbox.json' — direnv will not reload on config changes",
            ));
        }

        issues
    }

    fn check_secret(&self, trimmed: &str, rel: &str, lineno: usize) -> Option<ScannerIssue> {
        for pattern_str in &self.secret_patterns {
            if let Ok(re) = regex::Regex::new(pattern_str) {
                if re.is_match(trimmed) {
                    return Some(
                        ScannerIssue::new(
                            "envrc-no-hardcoded-secrets",
                            "error",
                            rel,
                            "Hardcoded secret value detected in .envrc — use dotenv_if_exists or source from external",
                        )
                        .at_line(lineno),
                    );
                }
            }
        }
        None
    }
}

fn check_absolute_path(trimmed: &str, rel: &str, lineno: usize) -> Option<ScannerIssue> {
    if trimmed.starts_with("path_prepend") || trimmed.starts_with("export PATH") {
        return None;
    }
    let re = regex::Regex::new(r#"(?:^|[ =])(/(?:usr|opt|nix|home|var|etc)/[^\s"']+)"#).ok()?;
    if let Some(caps) = re.captures(trimmed) {
        let abs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if !abs.is_empty() {
            return Some(
                ScannerIssue::new(
                    "envrc-no-absolute-paths",
                    "warning",
                    rel,
                    format!(
                        "Hardcoded absolute path '{}' in .envrc — use $HOME, relative paths, or $(command -v ...)",
                        abs
                    ),
                )
                .at_line(lineno),
            );
        }
    }
    None
}

fn default_secret_patterns() -> Vec<String> {
    vec![r#"^export\s+\w+=['"]?[^\s$({][^\s]*"#.to_string()]
}

impl Default for EnvrcContentScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clean_envrc_with_devbox_and_watch_file_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".envrc"),
            "use devbox\nwatch_file devbox.json\n",
        )?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_hardcoded_secret() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".envrc"),
            "use devbox\nexport API_KEY=sk_live_12345\n",
        )?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let secret = issues
            .iter()
            .find(|i| i.rule == "envrc-no-hardcoded-secrets")
            .expect("expected a hardcoded-secret issue");
        assert_eq!(secret.severity, "error");
        Ok(())
    }

    #[test]
    fn does_not_flag_command_substitution_as_secret() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".envrc"),
            "use devbox\nexport PATH=$(pwd)/bin:$PATH\n",
        )?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues
                .iter()
                .any(|i| i.rule == "envrc-no-hardcoded-secrets"),
            "command substitution should not be flagged as a secret"
        );
        Ok(())
    }

    #[test]
    fn flags_missing_devbox_directive() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".envrc"), "export FOO=bar\n")?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "envrc-uses-devbox"),
            "expected envrc-uses-devbox warning"
        );
        Ok(())
    }

    #[test]
    fn flags_direnv_allow_command() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".envrc"), "use devbox\ndirenv allow\n")?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "envrc-no-direnv-allow-rc"),
            "expected envrc-no-direnv-allow-rc info"
        );
        Ok(())
    }

    #[test]
    fn flags_missing_watch_file_when_using_devbox() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".envrc"), "use devbox\n")?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "envrc-watch-file-devbox"),
            "expected envrc-watch-file-devbox warning"
        );
        Ok(())
    }

    #[test]
    fn flags_absolute_path() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".envrc"),
            "use devbox\nsource /usr/local/bin/env.sh\n",
        )?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "envrc-no-absolute-paths"),
            "expected envrc-no-absolute-paths warning"
        );
        Ok(())
    }

    #[test]
    fn silent_on_repo_without_envrc() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# no envrc here\n")?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_envrc_emits_devbox_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".envrc"), "")?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "envrc-uses-devbox"),
            "empty .envrc should warn about missing devbox directive"
        );
        Ok(())
    }

    #[test]
    fn flake_based_envrc_does_not_require_watch_file_devbox() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join(".envrc"), "use flake\n")?;
        let scanner = EnvrcContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues.iter().any(|i| i.rule == "envrc-watch-file-devbox"),
            "flake-based envrc should not require watch_file devbox.json"
        );
        assert!(
            !issues.iter().any(|i| i.rule == "envrc-uses-devbox"),
            "flake-based envrc satisfies the devbox/flake requirement"
        );
        Ok(())
    }
}
