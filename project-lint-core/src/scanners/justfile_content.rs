//! Justfile content scanner — validates `justfile` / `Justfile` content for
//! required targets (quality, quality-full, ci, bootstrap), devbox wrapper
//! usage, no absolute paths, no forbidden commands (npx, bunx, yarn), and no
//! raw cargo calls in devbox projects.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;
use tracing::debug;

pub struct JustfileContentScanner {
    require_devbox_wrapper: bool,
    forbidden_commands: Vec<String>,
    required_targets: Vec<String>,
    excluded: Vec<String>,
}

impl JustfileContentScanner {
    pub fn new() -> Self {
        Self {
            require_devbox_wrapper: true,
            forbidden_commands: vec!["npx".to_string(), "bunx".to_string(), "yarn".to_string()],
            required_targets: vec![
                "quality".to_string(),
                "quality-full".to_string(),
                "ci".to_string(),
            ],
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_devbox_wrapper: bool,
        forbidden_commands: Vec<String>,
        required_targets: Vec<String>,
    ) -> Self {
        Self {
            require_devbox_wrapper,
            forbidden_commands,
            required_targets,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_devbox_wrapper: bool,
        forbidden_commands: Vec<String>,
        required_targets: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_devbox_wrapper,
            forbidden_commands,
            required_targets,
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
            if name != "justfile" && name != "Justfile" {
                continue;
            }
            issues.extend(self.scan_justfile(path, &rel_str));
        }

        Ok(issues)
    }

    fn scan_justfile(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        let defined_targets = extract_target_names(&content);

        for required in &self.required_targets {
            if !defined_targets.iter().any(|t| t == required) {
                let severity = if required == "quality" {
                    "error"
                } else {
                    "warning"
                };
                issues.push(ScannerIssue::new(
                    if required == "quality" {
                        "justfile-quality-target"
                    } else if required == "quality-full" {
                        "justfile-quality-full-target"
                    } else if required == "ci" {
                        "justfile-ci-target"
                    } else {
                        "justfile-required-target"
                    },
                    severity,
                    rel,
                    format!("justfile missing required target '{}'", required),
                ));
            }
        }

        if !defined_targets.iter().any(|t| t == "bootstrap") {
            issues.push(ScannerIssue::new(
                "justfile-bootstrap-target",
                "info",
                rel,
                "justfile missing 'bootstrap' target for first-time setup",
            ));
        }

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }

            if has_absolute_path(trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "justfile-no-absolute-paths",
                        "error",
                        rel,
                        "justfile contains hardcoded absolute path; use justfile_directory() or relative paths",
                    )
                    .at_line(i + 1),
                );
            }

            for cmd in &self.forbidden_commands {
                if line_contains_command(trimmed, cmd) {
                    issues.push(
                        ScannerIssue::new(
                            "justfile-no-npx-bunx-yarn",
                            "error",
                            rel,
                            format!(
                                "justfile uses forbidden command '{}'; use 'pnpm dlx' or 'pnpm exec' instead",
                                cmd
                            ),
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if self.require_devbox_wrapper && has_raw_cargo(trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "justfile-no-raw-cargo",
                        "warning",
                        rel,
                        "justfile calls 'cargo' directly; use 'devbox run -- cargo' to ensure correct toolchain",
                    )
                    .at_line(i + 1),
                );
            }
        }

        if self.require_devbox_wrapper && !content.contains("devbox run") {
            debug!("justfile has no devbox run usage at all");
        }

        issues
    }
}

impl Default for JustfileContentScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_target_names(content: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(colon_pos) = trimmed.find(':') {
            let target_part = &trimmed[..colon_pos];
            if target_part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                let name = target_part.trim().to_string();
                if !name.is_empty() && !targets.contains(&name) {
                    targets.push(name);
                }
            }
        }
    }
    targets
}

fn has_absolute_path(line: &str) -> bool {
    if line.starts_with("export ")
        || line.contains("justfile_directory()")
        || line.contains("env_var(")
    {
        return false;
    }
    if line.contains("/Users/") || line.contains("/home/") || line.contains("/tmp/") {
        return true;
    }
    if let Some(rest) = line.strip_prefix("cd ") {
        if rest.trim_start().starts_with('/') {
            return true;
        }
    }
    false
}

fn line_contains_command(line: &str, cmd: &str) -> bool {
    let patterns = [
        format!("{} ", cmd),
        format!("{}\t", cmd),
        format!("{}$", cmd),
    ];
    for p in &patterns {
        if line.contains(p) {
            let idx = line.find(p).unwrap();
            if idx == 0 || !line[..idx].chars().next().unwrap().is_alphanumeric() {
                return true;
            }
        }
    }
    false
}

fn has_raw_cargo(line: &str) -> bool {
    if line.contains("devbox run") {
        return false;
    }
    let patterns = [
        "cargo build",
        "cargo test",
        "cargo clippy",
        "cargo fmt",
        "cargo check",
        "cargo run",
        "cargo bench",
    ];
    patterns.iter().any(|p| line.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_justfile(dir: &Path, content: &str) {
        std::fs::write(dir.join("justfile"), content).unwrap();
    }

    fn valid_justfile() -> &'static str {
        "quality:\n  devbox run -- cargo fmt -- --check\n  devbox run -- cargo clippy\n  devbox run -- cargo test\n\
         quality-full: quality\n  devbox run -- cargo test --doc\n  devbox run -- cargo bench --no-run\n\
         ci: quality-full\n\
         bootstrap:\n  devbox install\n"
    }

    #[test]
    fn valid_justfile_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(&dir.path(), valid_justfile());
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_quality_target() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(&dir.path(), "build:\n  devbox run -- cargo build\n");
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "justfile-quality-target" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn flags_missing_quality_full_target() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(&dir.path(), "quality:\n  devbox run -- cargo test\n");
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "justfile-quality-full-target"));
        Ok(())
    }

    #[test]
    fn flags_absolute_paths() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(
            &dir.path(),
            "quality:\n  devbox run -- cargo test\nbuild:\n  cd /Users/foo/project && make\n",
        );
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "justfile-no-absolute-paths"));
        Ok(())
    }

    #[test]
    fn flags_forbidden_commands() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(
            &dir.path(),
            "quality:\n  devbox run -- cargo test\ndeploy:\n  npx vercel deploy\n",
        );
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "justfile-no-npx-bunx-yarn"));
        Ok(())
    }

    #[test]
    fn flags_raw_cargo() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(&dir.path(), "quality:\n  cargo test\n");
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "justfile-no-raw-cargo"));
        Ok(())
    }

    #[test]
    fn raw_cargo_not_flagged_when_devbox_disabled() -> Result<()> {
        let dir = TempDir::new()?;
        write_justfile(
            &dir.path(),
            "quality:\n  cargo test\nquality-full: quality\nci: quality-full\nbootstrap:\n  echo hi\n",
        );
        let scanner =
            JustfileContentScanner::with_config(false, vec![], vec!["quality".to_string()]);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "justfile-no-raw-cargo"));
        Ok(())
    }

    #[test]
    fn silent_when_no_justfile() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = JustfileContentScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn handles_capital_justfile() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("Justfile"),
            "quality:\n  devbox run -- cargo test\nquality-full: quality\nci: quality-full\nbootstrap:\n  devbox install\n",
        )?;
        let scanner = JustfileContentScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }
}
