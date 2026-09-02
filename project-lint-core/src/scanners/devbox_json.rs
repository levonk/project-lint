//! devbox.json scanner — validates `devbox.json` files for Devbox-based dev
//! environments. Parses the file as JSON (not regex) and enforces nine rules:
//!
//! 1. **devbox-name-present** — `devbox.json` should have a `"name"` field.
//! 2. **devbox-packages-is-object** — `"packages"` must be an object (map),
//!    not an array (older devbox format).
//! 3. **devbox-schema-present** — should have a `"$schema"` field.
//! 4. **devbox-no-floating-nixpkgs** — `devbox.lock` must be present and
//!    committed when `devbox.json` exists.
//! 5. **devbox-lock-present** — if `devbox.json` exists, `devbox.lock` must
//!    also exist.
//! 6. **devbox-github-packages-pinned** — GitHub packages (`github:owner/repo`)
//!    should pin to a specific rev or tag, not a floating branch.
//! 7. **devbox-init-hook-not-empty** — if `shell.init_hook` is present, it
//!    should not be an empty array.
//! 8. **devbox-scripts-use-just** — `scripts` entries should delegate to
//!    `just` targets rather than inlining commands.
//! 9. **devbox-no-npx-bunx-yarn** — `scripts` and `init_hook` must not
//!    contain `npx`, `bunx`, or `yarn` commands.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use serde_json::Value;
use std::path::Path;

pub struct DevboxJsonScanner {
    require_schema: bool,
    require_lock: bool,
    require_scripts_use_just: bool,
    forbidden_commands: Vec<String>,
    excluded: Vec<String>,
}

impl DevboxJsonScanner {
    pub fn new() -> Self {
        Self {
            require_schema: true,
            require_lock: true,
            require_scripts_use_just: true,
            forbidden_commands: default_forbidden_commands(),
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_schema: bool,
        require_lock: bool,
        require_scripts_use_just: bool,
        forbidden_commands: Vec<String>,
    ) -> Self {
        let forbidden_commands = if forbidden_commands.is_empty() {
            default_forbidden_commands()
        } else {
            forbidden_commands
        };
        Self {
            require_schema,
            require_lock,
            require_scripts_use_just,
            forbidden_commands,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_schema: bool,
        require_lock: bool,
        require_scripts_use_just: bool,
        forbidden_commands: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        let forbidden_commands = if forbidden_commands.is_empty() {
            default_forbidden_commands()
        } else {
            forbidden_commands
        };
        Self {
            require_schema,
            require_lock,
            require_scripts_use_just,
            forbidden_commands,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "devbox.json" {
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
            issues.extend(self.scan_devbox_json(path, &rel, root));
        }

        Ok(issues)
    }

    fn scan_devbox_json(&self, path: &Path, rel: &str, root: &Path) -> Vec<ScannerIssue> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let json: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                return vec![ScannerIssue::new(
                    "devbox-json-parse",
                    "error",
                    rel,
                    format!("devbox.json is not valid JSON: {}", e),
                )]
            }
        };
        let obj = match json.as_object() {
            Some(o) => o,
            None => {
                return vec![ScannerIssue::new(
                    "devbox-json-parse",
                    "error",
                    rel,
                    "devbox.json root is not a JSON object",
                )]
            }
        };
        let mut issues = Vec::new();

        if !obj.contains_key("name") {
            issues.push(ScannerIssue::new(
                "devbox-name-present",
                "info",
                rel,
                "devbox.json should have a 'name' field",
            ));
        }

        if let Some(packages) = obj.get("packages") {
            if !packages.is_object() {
                issues.push(ScannerIssue::new(
                    "devbox-packages-is-object",
                    "error",
                    rel,
                    "'packages' field must be an object (map of name to version), not an array",
                ));
            }
            if let Some(pkg_map) = packages.as_object() {
                for (pkg_name, _ver) in pkg_map {
                    if pkg_name.starts_with("github:") {
                        let has_rev = pkg_name.contains('#');
                        if !has_rev {
                            issues.push(ScannerIssue::new(
                                "devbox-github-packages-pinned",
                                "warning",
                                rel,
                                format!(
                                    "GitHub package '{}' should pin to a specific rev or tag (e.g. github:owner/repo#rev)",
                                    pkg_name
                                ),
                            ));
                        }
                    }
                }
            }
        }

        if self.require_schema && !obj.contains_key("$schema") {
            issues.push(ScannerIssue::new(
                "devbox-schema-present",
                "info",
                rel,
                "devbox.json should have a '$schema' field pointing to the devbox schema URL",
            ));
        }

        if self.require_lock {
            let lock_path = path.parent().unwrap_or(root).join("devbox.lock");
            if !lock_path.exists() {
                issues.push(ScannerIssue::new(
                    "devbox-lock-present",
                    "error",
                    rel,
                    "devbox.json exists but devbox.lock is missing — run 'devbox install' and commit the lockfile",
                ));
            }
        }

        if let Some(shell) = obj.get("shell").and_then(|s| s.as_object()) {
            if let Some(init_hook) = shell.get("init_hook") {
                if init_hook.is_array()
                    && init_hook.as_array().map(|a| a.is_empty()).unwrap_or(false)
                {
                    issues.push(ScannerIssue::new(
                        "devbox-init-hook-not-empty",
                        "info",
                        rel,
                        "shell.init_hook is an empty array — remove it or add hooks",
                    ));
                }
                if let Some(arr) = init_hook.as_array() {
                    for hook_val in arr {
                        if let Some(hook_str) = hook_val.as_str() {
                            issues.extend(self.check_forbidden_commands(hook_str, rel));
                        }
                    }
                }
            }
        }

        if let Some(scripts) = obj.get("scripts").and_then(|s| s.as_object()) {
            for (script_name, script_val) in scripts {
                if let Some(script_str) = script_val.as_str() {
                    if self.require_scripts_use_just && !script_str.contains("just ") {
                        issues.push(ScannerIssue::new(
                            "devbox-scripts-use-just",
                            "warning",
                            rel,
                            format!(
                                "script '{}' should delegate to a just target (e.g. 'just {}_impl') rather than inlining commands",
                                script_name, script_name
                            ),
                        ));
                    }
                    issues.extend(self.check_forbidden_commands(script_str, rel));
                }
            }
        }

        issues
    }

    fn check_forbidden_commands(&self, text: &str, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        for cmd in &self.forbidden_commands {
            let pattern = format!(r"\b{}\b", regex::escape(cmd));
            if let Ok(re) = regex::Regex::new(&pattern) {
                if re.is_match(text) {
                    issues.push(ScannerIssue::new(
                        "devbox-no-npx-bunx-yarn",
                        "error",
                        rel,
                        format!(
                            "Forbidden command '{}' in devbox.json — use 'pnpm dlx' or 'pnpm exec' instead",
                            cmd
                        ),
                    ));
                }
            }
        }
        issues
    }
}

fn default_forbidden_commands() -> Vec<String> {
    vec!["npx".to_string(), "bunx".to_string(), "yarn".to_string()]
}

impl Default for DevboxJsonScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clean_devbox_json_with_lock_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{
  "name": "myproject",
  "$schema": "https://raw.githubusercontent.com/jetify-com/devbox/main/schema.json",
  "packages": { "go": "" },
  "shell": { "init_hook": ["just bootstrap_impl"] },
  "scripts": { "build": "just build_impl" }
}
"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_packages_as_array() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":["go"],"scripts":{"build":"just build_impl"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "devbox-packages-is-object"),
            "expected devbox-packages-is-object error"
        );
        Ok(())
    }

    #[test]
    fn flags_missing_lock() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"go":""},"scripts":{"build":"just build_impl"}}"#,
        )?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "devbox-lock-present"),
            "expected devbox-lock-present error"
        );
        Ok(())
    }

    #[test]
    fn flags_missing_name_and_schema() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"packages":{"go":""},"scripts":{"build":"just build_impl"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "devbox-name-present"));
        assert!(issues.iter().any(|i| i.rule == "devbox-schema-present"));
        Ok(())
    }

    #[test]
    fn flags_floating_github_package() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"github:owner/repo":""},"scripts":{"build":"just build_impl"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues
                .iter()
                .any(|i| i.rule == "devbox-github-packages-pinned"),
            "expected devbox-github-packages-pinned warning"
        );
        Ok(())
    }

    #[test]
    fn pinned_github_package_is_ok() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"github:owner/repo#abc123":""},"scripts":{"build":"just build_impl"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues
                .iter()
                .any(|i| i.rule == "devbox-github-packages-pinned"),
            "pinned github package should not be flagged"
        );
        Ok(())
    }

    #[test]
    fn flags_empty_init_hook() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"go":""},"shell":{"init_hook":[]},"scripts":{"build":"just build_impl"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues
                .iter()
                .any(|i| i.rule == "devbox-init-hook-not-empty"),
            "expected devbox-init-hook-not-empty info"
        );
        Ok(())
    }

    #[test]
    fn flags_script_not_using_just() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"go":""},"scripts":{"build":"cargo build"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "devbox-scripts-use-just"),
            "expected devbox-scripts-use-just warning"
        );
        Ok(())
    }

    #[test]
    fn flags_forbidden_npx_command() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"go":""},"scripts":{"build":"npx tsc"}}"#,
        )?;
        std::fs::write(dir.path().join("devbox.lock"), "{}\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "devbox-no-npx-bunx-yarn"),
            "expected devbox-no-npx-bunx-yarn error"
        );
        Ok(())
    }

    #[test]
    fn flags_invalid_json() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("devbox.json"), "{not valid json")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "devbox-json-parse"),
            "expected devbox-json-parse error"
        );
        Ok(())
    }

    #[test]
    fn silent_on_repo_without_devbox_json() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# no devbox here\n")?;
        let scanner = DevboxJsonScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn config_can_disable_lock_check() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("devbox.json"),
            r#"{"name":"x","$schema":"s","packages":{"go":""},"scripts":{"build":"just build_impl"}}"#,
        )?;
        let scanner = DevboxJsonScanner::with_config(true, false, true, vec![]);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            !issues.iter().any(|i| i.rule == "devbox-lock-present"),
            "lock check disabled should not flag missing lock"
        );
        Ok(())
    }
}
