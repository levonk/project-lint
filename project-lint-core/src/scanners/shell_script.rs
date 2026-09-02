//! Shell script scanner — validates `*.sh` / `*.bash` files against the
//! shell-scripting best-practices bundle. Enforces shebang form, strict mode,
//! guarded PATH additions, `exec` for final long-lived commands, dirty-git
//! gates for destructive scripts, dry-run capability, bounded timeouts, no
//! hardcoded home paths, forbidden package-manager commands, and `devbox run`
//! usage in devbox projects.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;
use tracing::debug;

/// Default list of package-manager commands forbidden in shell scripts on the
/// host (use `pnpm dlx` instead). The container exception does not apply to
/// `.sh` files executed on the host.
pub const DEFAULT_FORBIDDEN_COMMANDS: &[&str] = &["npx", "bunx", "yarn dlx"];

/// Build-tool commands that should be invoked via `devbox run --` when the
/// project has a `devbox.json`.
const DEVBOX_WRAPPED_TOOLS: &[&str] = &[
    "cargo", "npm", "pnpm", "yarn", "just", "go", "python", "python3", "node", "tsc", "eslint",
    "prettier", "ruff", "black", "mypy", "pytest",
];

/// Commands considered destructive — scripts containing them should gate on
/// dirty git state and offer a dry-run path.
const DESTRUCTIVE_COMMANDS: &[&str] = &[
    "rm -rf",
    "git push",
    "git reset --hard",
    "git clean -fd",
    "git checkout --",
    "git rebase",
    "dropdb",
    "DROP TABLE",
    "truncate",
];

pub struct ShellScriptScanner {
    require_shebang: bool,
    require_strict_mode: bool,
    forbid_hardcoded_home: bool,
    forbidden_commands: Vec<String>,
    require_devbox_run: bool,
    excluded: Vec<String>,
}

impl ShellScriptScanner {
    pub fn new() -> Self {
        Self {
            require_shebang: true,
            require_strict_mode: true,
            forbid_hardcoded_home: true,
            forbidden_commands: DEFAULT_FORBIDDEN_COMMANDS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            require_devbox_run: false,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_shebang: bool,
        require_strict_mode: bool,
        forbid_hardcoded_home: bool,
        forbidden_commands: Vec<String>,
        require_devbox_run: bool,
    ) -> Self {
        Self {
            require_shebang,
            require_strict_mode,
            forbid_hardcoded_home,
            forbidden_commands,
            require_devbox_run,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_shebang: bool,
        require_strict_mode: bool,
        forbid_hardcoded_home: bool,
        forbidden_commands: Vec<String>,
        require_devbox_run: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_shebang,
            require_strict_mode,
            forbid_hardcoded_home,
            forbidden_commands,
            require_devbox_run,
            excluded,
        }
    }

    /// Scan a project root for `*.sh` / `*.bash` files and lint each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let has_devbox = root.join("devbox.json").exists();

        for entry in walk_project(root, &self.excluded, 6).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".sh") && !name.ends_with(".bash") {
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
            debug!("shell_script scanning {}", rel);
            issues.extend(self.scan_script(path, &rel, has_devbox));
        }

        Ok(issues)
    }

    fn scan_script(&self, path: &Path, rel: &str, has_devbox: bool) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        if content.trim().is_empty() {
            return issues;
        }

        // Rule: sh-shebang
        if self.require_shebang {
            if let Some(first) = lines.first() {
                let t = first.trim();
                if t.starts_with("#!") {
                    if t != "#!/usr/bin/env bash" {
                        issues.push(
                            ScannerIssue::new(
                                "sh-shebang",
                                "warning",
                                rel,
                                "shebang should be '#!/usr/bin/env bash'",
                            )
                            .at_line(1),
                        );
                    }
                } else {
                    issues.push(
                        ScannerIssue::new(
                            "sh-shebang",
                            "warning",
                            rel,
                            "missing shebang (expected '#!/usr/bin/env bash')",
                        )
                        .at_line(1),
                    );
                }
            }
        }

        // Rule: sh-strict-mode
        if self.require_strict_mode {
            let has_strict = lines
                .iter()
                .take(10)
                .any(|l| l.trim().contains("set -euo pipefail"));
            if !has_strict {
                issues.push(ScannerIssue::new(
                    "sh-strict-mode",
                    "warning",
                    rel,
                    "missing 'set -euo pipefail' after shebang",
                ));
            }
        }

        let mut has_dirty_git_check = false;
        let mut has_dry_run = false;
        let mut is_destructive = false;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let lineno = i + 1;

            // Rule: sh-path-addition-guard
            if self.is_path_addition(trimmed) {
                if !self.has_path_guard(&lines, i) {
                    issues.push(
                        ScannerIssue::new(
                            "sh-path-addition-guard",
                            "warning",
                            rel,
                            "PATH addition should be guarded against duplicates (case \":$PATH:\" in ...)",
                        )
                        .at_line(lineno),
                    );
                }
            }

            // Rule: sh-no-hardcoded-home
            if self.forbid_hardcoded_home && self.has_hardcoded_home(trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "sh-no-hardcoded-home",
                        "warning",
                        rel,
                        "hardcoded home path detected; use $HOME instead",
                    )
                    .at_line(lineno),
                );
            }

            // Rule: sh-no-npx-bunx-yarn
            for cmd in &self.forbidden_commands {
                if self.uses_forbidden_command(trimmed, cmd) {
                    issues.push(
                        ScannerIssue::new(
                            "sh-no-npx-bunx-yarn",
                            "error",
                            rel,
                            format!(
                                "forbidden command '{}' in shell script; use 'pnpm dlx'",
                                cmd
                            ),
                        )
                        .at_line(lineno),
                    );
                }
            }

            // Rule: sh-uses-devbox-run
            if has_devbox && self.require_devbox_run {
                if let Some(tool) = self.bare_devbox_tool(trimmed) {
                    issues.push(
                        ScannerIssue::new(
                            "sh-uses-devbox-run",
                            "warning",
                            rel,
                            format!(
                                "build tool '{}' in a devbox project should be invoked via 'devbox run --'",
                                tool
                            ),
                        )
                        .at_line(lineno),
                    );
                }
            }

            // Track destructive commands for git-cleanliness and dry-run gates.
            if DESTRUCTIVE_COMMANDS.iter().any(|d| trimmed.contains(d)) {
                is_destructive = true;
            }
            if trimmed.contains("git diff --quiet")
                || trimmed.contains("--dry-run")
                || trimmed.contains("-n")
                || trimmed.contains("DRY_RUN")
                || trimmed.contains("dry_run")
            {
                has_dry_run = true;
            }
            if trimmed.contains("git diff --quiet")
                || trimmed.contains("git status --porcelain")
                || trimmed.contains("dirty")
            {
                has_dirty_git_check = true;
            }

            // Rule: sh-bounded-timeout (long-running ops)
            if self.is_long_running(trimmed) && !trimmed.contains("timeout") {
                issues.push(
                    ScannerIssue::new(
                        "sh-bounded-timeout",
                        "info",
                        rel,
                        "long-running operation should be wrapped in 'timeout'",
                    )
                    .at_line(lineno),
                );
            }
        }

        // Rule: sh-git-cleanliness-gate + sh-dry-run-first (script-level)
        if is_destructive {
            if !has_dirty_git_check {
                issues.push(ScannerIssue::new(
                    "sh-git-cleanliness-gate",
                    "warning",
                    rel,
                    "destructive script should check for dirty git state before proceeding",
                ));
            }
            if !has_dry_run {
                issues.push(ScannerIssue::new(
                    "sh-dry-run-first",
                    "warning",
                    rel,
                    "destructive script should support a --dry-run path",
                ));
            }
        }

        // Rule: sh-exec-final-command
        if let Some(last_cmd_line) = self.last_command_line(&lines) {
            if self.is_long_lived_command(&lines[last_cmd_line.0])
                && !lines[last_cmd_line.0].trim().starts_with("exec ")
            {
                issues.push(
                    ScannerIssue::new(
                        "sh-exec-final-command",
                        "info",
                        rel,
                        "final long-lived command should use 'exec' to replace the shell process",
                    )
                    .at_line(last_cmd_line.1),
                );
            }
        }

        issues
    }

    fn is_path_addition(&self, line: &str) -> bool {
        let t = line.trim();
        (t.starts_with("export PATH=") || t.starts_with("export PATH:=") || t.starts_with("PATH="))
            && t.contains("$PATH")
    }

    fn has_path_guard(&self, lines: &[&str], idx: usize) -> bool {
        let guard_start = if idx > 0 { idx - 1 } else { 0 };
        let guard_end = (idx + 4).min(lines.len());
        for w in &lines[guard_start..guard_end] {
            if w.contains("case") && w.contains(":$PATH:") {
                return true;
            }
        }
        false
    }

    fn has_hardcoded_home(&self, line: &str) -> bool {
        let l = line;
        if l.contains("/Users/") && !l.contains("$HOME") {
            let after = l.split("/Users/").nth(1).unwrap_or("");
            if let Some(user) = after.split('/').next() {
                if !user.is_empty() && !user.starts_with('$') {
                    return true;
                }
            }
        }
        if l.contains("/home/") && !l.contains("$HOME") {
            let after = l.split("/home/").nth(1).unwrap_or("");
            if let Some(user) = after.split('/').next() {
                if !user.is_empty() && !user.starts_with('$') {
                    return true;
                }
            }
        }
        if l.contains("C:\\Users\\") {
            return true;
        }
        false
    }

    fn uses_forbidden_command(&self, line: &str, cmd: &str) -> bool {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(cmd) {
            return rest.is_empty()
                || rest.starts_with(' ')
                || rest.starts_with('\t')
                || rest.starts_with('"');
        }
        if line.contains(&format!(" {} ", cmd)) || line.contains(&format!("\t{} ", cmd)) {
            return true;
        }
        if line.contains(&format!("`{}", cmd)) || line.contains(&format!("$({}", cmd)) {
            return true;
        }
        false
    }

    fn bare_devbox_tool(&self, line: &str) -> Option<&'static str> {
        let trimmed = line.trim_start();
        for tool in DEVBOX_WRAPPED_TOOLS {
            if let Some(rest) = trimmed.strip_prefix(tool) {
                if rest.is_empty()
                    || rest.starts_with(' ')
                    || rest.starts_with('\t')
                    || rest.starts_with('"')
                {
                    if !trimmed.starts_with("devbox run") {
                        return Some(tool);
                    }
                }
            }
        }
        None
    }

    fn is_long_running(&self, line: &str) -> bool {
        let t = line.trim();
        t.starts_with("npm ")
            || t.starts_with("pnpm ")
            || t.starts_with("yarn ")
            || t.starts_with("cargo ")
            || t.starts_with("go ")
            || t.starts_with("docker build")
            || t.starts_with("docker compose")
            || t.starts_with("kubectl ")
            || t.starts_with("terraform ")
            || t.starts_with("ansible ")
            || t.starts_with("pytest")
            || t.starts_with("jest")
    }

    fn is_long_lived_command(&self, line: &str) -> bool {
        let t = line.trim();
        if t.starts_with("exec ") {
            return false;
        }
        t.starts_with("npm ")
            || t.starts_with("pnpm ")
            || t.starts_with("yarn ")
            || t.starts_with("cargo run")
            || t.starts_with("node ")
            || t.starts_with("python ")
            || t.starts_with("python3 ")
            || t.starts_with("go run")
            || t.starts_with("docker compose up")
            || t.starts_with("docker compose ")
    }

    fn last_command_line<'a>(&self, lines: &'a [&'a str]) -> Option<(usize, usize)> {
        for (i, line) in lines.iter().enumerate().rev() {
            let t = line.trim();
            if t.is_empty()
                || t.starts_with('#')
                || t.starts_with("fi")
                || t.starts_with("done")
                || t.starts_with("}")
                || t.starts_with("esac")
            {
                continue;
            }
            return Some((i, i + 1));
        }
        None
    }
}

impl Default for ShellScriptScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_sh(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    fn clean_script() -> &'static str {
        "#!/usr/bin/env bash\nset -euo pipefail\n\necho hello\n"
    }

    #[test]
    fn clean_script_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(&dir.path(), "run.sh", clean_script());
        let scanner = ShellScriptScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_bad_shebang() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/bin/bash\nset -euo pipefail\necho hi\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-shebang"));
        Ok(())
    }

    #[test]
    fn flags_missing_shebang() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(&dir.path(), "run.sh", "set -euo pipefail\necho hi\n");
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-shebang"));
        Ok(())
    }

    #[test]
    fn flags_missing_strict_mode() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(&dir.path(), "run.sh", "#!/usr/bin/env bash\necho hi\n");
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-strict-mode"));
        Ok(())
    }

    #[test]
    fn flags_unguarded_path_addition() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nexport PATH=\"$HOME/bin:$PATH\"\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-path-addition-guard"));
        Ok(())
    }

    #[test]
    fn guarded_path_addition_passes() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ncase \":$PATH:\" in *\":$HOME/bin:\"*) ;; *) export PATH=\"$HOME/bin:$PATH\";; esac\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues.iter().any(|i| i.rule == "sh-path-addition-guard"),
            "issues: {:?}",
            issues
        );
        Ok(())
    }

    #[test]
    fn flags_hardcoded_home() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ncd /Users/micro/projects\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-no-hardcoded-home"));
        Ok(())
    }

    #[test]
    fn home_var_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ncd \"$HOME/projects\"\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "sh-no-hardcoded-home"));
        Ok(())
    }

    #[test]
    fn flags_npx_command() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nnpx prettier --write .\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sh-no-npx-bunx-yarn" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn flags_bunx_command() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nbunx prettier --write .\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-no-npx-bunx-yarn"));
        Ok(())
    }

    #[test]
    fn flags_yarn_dlx_command() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nyarn dlx prettier --write .\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-no-npx-bunx-yarn"));
        Ok(())
    }

    #[test]
    fn flags_destructive_without_git_gate() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nrm -rf dist\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-git-cleanliness-gate"));
        assert!(issues.iter().any(|i| i.rule == "sh-dry-run-first"));
        Ok(())
    }

    #[test]
    fn destructive_with_gates_passes() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nif ! git diff --quiet; then echo dirty; exit 1; fi\nrm -rf dist --dry-run\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues.iter().any(|i| i.rule == "sh-git-cleanliness-gate"),
            "issues: {:?}",
            issues
        );
        assert!(!issues.iter().any(|i| i.rule == "sh-dry-run-first"));
        Ok(())
    }

    #[test]
    fn flags_long_running_without_timeout() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nnpm run build\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-bounded-timeout"));
        Ok(())
    }

    #[test]
    fn long_running_with_timeout_passes() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ntimeout 300 npm run build\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "sh-bounded-timeout"));
        Ok(())
    }

    #[test]
    fn flags_final_long_lived_without_exec() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nnode server.js\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-exec-final-command"));
        Ok(())
    }

    #[test]
    fn final_long_lived_with_exec_passes() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\nexec node server.js\n",
        );
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "sh-exec-final-command"));
        Ok(())
    }

    #[test]
    fn flags_bare_tool_in_devbox_project() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("devbox.json"), "{}").unwrap();
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ncargo build --release\n",
        );
        let scanner = ShellScriptScanner::with_config(true, true, true, Vec::new(), true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-uses-devbox-run"));
        Ok(())
    }

    #[test]
    fn devbox_run_wrapped_tool_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("devbox.json"), "{}").unwrap();
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ndevbox run -- cargo build --release\n",
        );
        let scanner = ShellScriptScanner::with_config(true, true, true, Vec::new(), true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "sh-uses-devbox-run"));
        Ok(())
    }

    #[test]
    fn no_devbox_json_no_devbox_run_flag() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(
            &dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nset -euo pipefail\ncargo build --release\n",
        );
        let scanner = ShellScriptScanner::with_config(true, true, true, Vec::new(), true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "sh-uses-devbox-run"));
        Ok(())
    }

    #[test]
    fn empty_file_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(&dir.path(), "empty.sh", "");
        let scanner = ShellScriptScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn bash_extension_scanned() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(&dir.path(), "run.bash", "#!/bin/bash\necho hi\n");
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sh-shebang"));
        Ok(())
    }

    #[test]
    fn config_can_disable_shebang_and_strict() -> Result<()> {
        let dir = TempDir::new()?;
        write_sh(&dir.path(), "run.sh", "#!/bin/bash\necho hi\n");
        let scanner = ShellScriptScanner::with_config(false, false, false, Vec::new(), false);
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn respects_excluded_dirs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        write_sh(&dir.path().join("target"), "bad.sh", "#!/bin/sh\necho hi\n");
        let scanner = ShellScriptScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues.iter().any(|i| i.file.contains("target/")),
            "should not scan target/: {:?}",
            issues
        );
        Ok(())
    }

    #[test]
    fn default_forbidden_commands_contains_npx_bunx() {
        assert!(DEFAULT_FORBIDDEN_COMMANDS.contains(&"npx"));
        assert!(DEFAULT_FORBIDDEN_COMMANDS.contains(&"bunx"));
        assert!(DEFAULT_FORBIDDEN_COMMANDS.contains(&"yarn dlx"));
    }
}
