//! Process-compose scanner — validates `process-compose.yaml` / `.yml` files
//! for valid commands, health checks, restart policies, no absolute paths,
//! and devbox usage.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, detect_yaml_secrets, is_excluded_rel, walk_project, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

pub struct ProcessComposeScanner {
    require_health_check: bool,
    require_devbox: bool,
    excluded: Vec<String>,
}

impl ProcessComposeScanner {
    pub fn new() -> Self {
        Self {
            require_health_check: true,
            require_devbox: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_health_check: bool, require_devbox: bool) -> Self {
        Self {
            require_health_check,
            require_devbox,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_health_check: bool,
        require_devbox: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_health_check,
            require_devbox,
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
            if name != "process-compose.yaml" && name != "process-compose.yml" {
                continue;
            }
            issues.extend(self.scan_process_compose(path, &rel_str));
        }

        Ok(issues)
    }

    fn scan_process_compose(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        let file: ProcessComposeFile = match serde_yaml::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                debug!("failed to parse process-compose file {:?}: {}", path, e);
                issues.push(ScannerIssue::new(
                    "process-compose-parse-error",
                    "error",
                    rel,
                    format!("invalid YAML: {}", e),
                ));
                return issues;
            }
        };

        for (line, msg) in detect_yaml_secrets(&content) {
            issues
                .push(ScannerIssue::new("yaml-hardcoded-secret", "error", rel, msg).at_line(line));
        }

        let processes = match file.processes {
            Some(p) => p,
            None => {
                issues.push(ScannerIssue::new(
                    "process-compose-missing-processes",
                    "warning",
                    rel,
                    "process-compose file missing top-level 'processes:' key",
                ));
                return issues;
            }
        };
        for (name, proc) in &processes {
            if proc.command.is_none() || proc.command.as_ref().map(|c| c.is_empty()).unwrap_or(true)
            {
                issues.push(ScannerIssue::new(
                    "process-compose-valid-commands",
                    "error",
                    rel,
                    format!("process '{}' missing 'command' field", name),
                ));
            }

            if let Some(ref cmd) = proc.command {
                if cmd.starts_with('/') {
                    issues.push(ScannerIssue::new(
                        "process-compose-no-absolute-paths",
                        "warning",
                        rel,
                        format!("process '{}' uses absolute path in command", name),
                    ));
                }

                if self.require_devbox && !cmd.contains("devbox run") {
                    issues.push(ScannerIssue::new(
                        "process-compose-uses-devbox",
                        "warning",
                        rel,
                        format!("process '{}' does not use 'devbox run --' prefix", name),
                    ));
                }
            }

            if self.require_health_check && proc.health_check.is_none() {
                issues.push(ScannerIssue::new(
                    "process-compose-health-check",
                    "warning",
                    rel,
                    format!("process '{}' missing 'health_check'", name),
                ));
            }

            if proc.restart_policy.is_none() {
                issues.push(ScannerIssue::new(
                    "process-compose-restart-policy",
                    "info",
                    rel,
                    format!("process '{}' missing 'restart_policy'", name),
                ));
            }
        }

        issues
    }
}

impl Default for ProcessComposeScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct ProcessComposeFile {
    #[serde(default)]
    processes: Option<HashMap<String, ProcessEntry>>,
}

#[derive(Debug, Deserialize)]
struct ProcessEntry {
    #[serde(default)]
    command: Option<String>,
    #[serde(default, rename = "health_check")]
    health_check: Option<serde_yaml::Value>,
    #[serde(default, rename = "restart_policy")]
    restart_policy: Option<serde_yaml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_process_compose(dir: &Path, content: &str) {
        std::fs::write(dir.join("process-compose.yaml"), content).unwrap();
    }

    fn valid_process_compose() -> String {
        "version: '0.5'\nprocesses:\n  web:\n    command: devbox run -- python -m http.server\n    health_check:\n      test: curl -f http://localhost:8000\n    restart_policy:\n      max_restarts: 3\n".to_string()
    }

    #[test]
    fn valid_process_compose_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(&dir.path(), &valid_process_compose());
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_command() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    health_check:\n      test: echo ok\n    restart_policy:\n      max_restarts: 3\n",
        );
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "process-compose-valid-commands"));
        Ok(())
    }

    #[test]
    fn flags_missing_health_check() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    command: devbox run -- python -m http.server\n    restart_policy:\n      max_restarts: 3\n",
        );
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "process-compose-health-check"));
        Ok(())
    }

    #[test]
    fn flags_missing_restart_policy() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    command: devbox run -- python -m http.server\n    health_check:\n      test: echo ok\n",
        );
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "process-compose-restart-policy"));
        Ok(())
    }

    #[test]
    fn flags_absolute_path_in_command() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    command: /usr/bin/python -m http.server\n    health_check:\n      test: echo ok\n    restart_policy:\n      max_restarts: 3\n",
        );
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "process-compose-no-absolute-paths"));
        Ok(())
    }

    #[test]
    fn flags_missing_devbox_usage() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    command: python -m http.server\n    health_check:\n      test: echo ok\n    restart_policy:\n      max_restarts: 3\n",
        );
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "process-compose-uses-devbox"));
        Ok(())
    }

    #[test]
    fn devbox_not_required_when_disabled() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    command: python -m http.server\n    health_check:\n      test: echo ok\n    restart_policy:\n      max_restarts: 3\n",
        );
        let scanner = ProcessComposeScanner::with_config(true, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "process-compose-uses-devbox"));
        Ok(())
    }

    #[test]
    fn silent_when_no_process_compose() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# hello\n")?;
        let scanner = ProcessComposeScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_invalid_yaml() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(&dir.path(), "version: '0.5\nprocesses: [bad: yaml\n");
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "process-compose-parse-error"));
        Ok(())
    }

    #[test]
    fn flags_missing_processes_key() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(&dir.path(), "version: '0.5'\n");
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| { i.rule == "process-compose-missing-processes" && i.severity == "warning" }));
        Ok(())
    }

    #[test]
    fn flags_hardcoded_secret_in_process_compose() -> Result<()> {
        let dir = TempDir::new()?;
        write_process_compose(
            &dir.path(),
            "version: '0.5'\nprocesses:\n  web:\n    command: devbox run -- python -m http.server\n    health_check:\n      test: echo ok\n    restart_policy:\n      max_restarts: 3\n    env:\n      API_TOKEN: abc123\n",
        );
        let scanner = ProcessComposeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "yaml-hardcoded-secret" && i.severity == "error"));
        Ok(())
    }
}
