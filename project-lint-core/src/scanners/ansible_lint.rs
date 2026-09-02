//! Ansible lint scanner — validates Ansible playbooks and ansible.cfg for
//! become at play level, hardcoded vault passwords, task names, command/shell
//! module usage, host key checking, and vault password file paths.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct AnsibleLintScanner {
    forbid_host_key_checking: bool,
    require_task_names: bool,
    excluded: Vec<String>,
}

impl AnsibleLintScanner {
    pub fn new() -> Self {
        Self {
            forbid_host_key_checking: true,
            require_task_names: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(forbid_host_key_checking: bool, require_task_names: bool) -> Self {
        Self {
            forbid_host_key_checking,
            require_task_names,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        forbid_host_key_checking: bool,
        require_task_names: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            forbid_host_key_checking,
            require_task_names,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }

            if name == "ansible.cfg" {
                issues.extend(self.scan_ansible_cfg(path, &rel));
                continue;
            }

            let is_playbook = name.ends_with(".yml") || name.ends_with(".yaml");
            if !is_playbook {
                continue;
            }

            let rel_lower = rel.to_lowercase();
            let in_ansible_dir = rel_lower.contains("ansible/")
                || rel_lower.contains("playbooks/")
                || rel_lower.contains("roles/")
                || rel_lower.starts_with("ansible")
                || rel_lower.starts_with("playbooks")
                || rel_lower.starts_with("roles");

            if !in_ansible_dir {
                continue;
            }

            issues.extend(self.scan_playbook(path, &rel));
        }

        Ok(issues)
    }

    fn scan_playbook(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut in_tasks = false;
        let mut task_indent = 0usize;
        let mut current_task_has_name = false;
        let mut current_task_start_line = 0usize;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let indent = line.len() - line.trim_start().len();

            if trimmed.contains("ansible_vault_password") || trimmed.contains("vault_password_file")
            {
                if trimmed.contains(':') {
                    let value = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
                    if value.starts_with('"') || value.starts_with('\'') {
                        issues.push(
                            ScannerIssue::new(
                                "ansible-no-hardcoded-vault-password",
                                "error",
                                rel,
                                "hardcoded vault password path — use env var or ansible-vault",
                            )
                            .at_line(i + 1),
                        );
                    }
                }
            }

            if trimmed.starts_with("- name:") || trimmed.starts_with("- name :") {
                if self.require_task_names
                    && in_tasks
                    && !current_task_has_name
                    && current_task_start_line > 0
                {
                    issues.push(
                        ScannerIssue::new(
                            "ansible-task-name-present",
                            "warning",
                            rel,
                            "task missing 'name:' field for readability",
                        )
                        .at_line(current_task_start_line),
                    );
                }
                current_task_has_name = true;
                current_task_start_line = i + 1;
                continue;
            }

            if trimmed == "tasks:" || trimmed.starts_with("tasks:") {
                in_tasks = true;
                task_indent = indent;
                current_task_has_name = true;
                current_task_start_line = 0;
                continue;
            }

            if in_tasks && !trimmed.is_empty() && indent <= task_indent && !trimmed.starts_with('-')
            {
                in_tasks = false;
            }

            if in_tasks && trimmed.starts_with('-') && indent <= task_indent + 2 {
                if self.require_task_names && !current_task_has_name && current_task_start_line > 0
                {
                    issues.push(
                        ScannerIssue::new(
                            "ansible-task-name-present",
                            "warning",
                            rel,
                            "task missing 'name:' field for readability",
                        )
                        .at_line(current_task_start_line),
                    );
                }
                current_task_has_name = false;
                current_task_start_line = i + 1;
            }

            if in_tasks {
                if trimmed.starts_with("command:")
                    || trimmed.starts_with("shell:")
                    || trimmed.starts_with("ansible.builtin.command:")
                    || trimmed.starts_with("ansible.builtin.shell:")
                {
                    issues.push(
                        ScannerIssue::new(
                            "ansible-no-command-shell",
                            "info",
                            rel,
                            "avoid 'command'/'shell' modules when a dedicated module exists",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if trimmed.starts_with("become:") {
                let value = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
                if value == "true" || value == "yes" {
                    let is_play_level = !in_tasks;
                    if is_play_level {
                        issues.push(
                            ScannerIssue::new(
                                "ansible-no-become-true-at-play",
                                "warning",
                                rel,
                                "'become: true' at play level — set at task level for granularity",
                            )
                            .at_line(i + 1),
                        );
                    }
                }
            }
        }

        if self.require_task_names
            && in_tasks
            && !current_task_has_name
            && current_task_start_line > 0
        {
            issues.push(
                ScannerIssue::new(
                    "ansible-task-name-present",
                    "warning",
                    rel,
                    "task missing 'name:' field for readability",
                )
                .at_line(current_task_start_line),
            );
        }

        issues
    }

    fn scan_ansible_cfg(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() || trimmed.starts_with('[') {
                continue;
            }

            if self.forbid_host_key_checking {
                if trimmed.to_lowercase().contains("host_key_checking")
                    && trimmed.to_lowercase().contains("false")
                {
                    issues.push(
                        ScannerIssue::new(
                            "ansible-cfg-no-host-key-checking",
                            "error",
                            rel,
                            "ansible.cfg sets host_key_checking = False — security risk",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if trimmed.to_lowercase().contains("vault_password_file") {
                let value = trimmed.splitn(2, '=').nth(1).unwrap_or("").trim();
                if !value.is_empty() && !value.contains("${") && !value.contains("$(") {
                    issues.push(
                        ScannerIssue::new(
                            "ansible-cfg-vault-password-file",
                            "error",
                            rel,
                            "ansible.cfg sets vault_password_file to a committed path — use env var",
                        )
                        .at_line(i + 1),
                    );
                }
            }
        }

        issues
    }
}

impl Default for AnsibleLintScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn silent_when_no_ansible_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = AnsibleLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_become_true_at_play_level() -> Result<()> {
        let dir = TempDir::new()?;
        let ansible_dir = dir.path().join("ansible");
        std::fs::create_dir_all(&ansible_dir)?;
        std::fs::write(
            ansible_dir.join("playbook.yml"),
            "---\n- hosts: all\n  become: true\n  tasks:\n    - name: install nginx\n      apt: name=nginx\n",
        )?;
        let scanner = AnsibleLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "ansible-no-become-true-at-play"));
        Ok(())
    }

    #[test]
    fn flags_task_missing_name() -> Result<()> {
        let dir = TempDir::new()?;
        let playbooks_dir = dir.path().join("playbooks");
        std::fs::create_dir_all(&playbooks_dir)?;
        std::fs::write(
            playbooks_dir.join("site.yml"),
            "---\n- hosts: all\n  tasks:\n    - apt: name=nginx\n",
        )?;
        let scanner = AnsibleLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "ansible-task-name-present"));
        Ok(())
    }

    #[test]
    fn flags_host_key_checking_in_cfg() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("ansible.cfg"),
            "[defaults]\nhost_key_checking = False\n",
        )?;
        let scanner = AnsibleLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "ansible-cfg-no-host-key-checking"));
        Ok(())
    }

    #[test]
    fn flags_vault_password_file_in_cfg() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("ansible.cfg"),
            "[defaults]\nvault_password_file = /home/user/.vault_pass\n",
        )?;
        let scanner = AnsibleLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "ansible-cfg-vault-password-file"));
        Ok(())
    }

    #[test]
    fn clean_playbook_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let ansible_dir = dir.path().join("ansible");
        std::fs::create_dir_all(&ansible_dir)?;
        std::fs::write(
            ansible_dir.join("playbook.yml"),
            "---\n- hosts: all\n  tasks:\n    - name: install nginx\n      apt: name=nginx\n",
        )?;
        let scanner = AnsibleLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.is_empty(),
            "expected no issues, got: {:?}",
            issues.iter().map(|i| &i.rule).collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn config_can_disable_checks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("ansible.cfg"),
            "[defaults]\nhost_key_checking = False\n",
        )?;
        let scanner = AnsibleLintScanner::with_config(false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "ansible-cfg-no-host-key-checking"));
        Ok(())
    }

    #[test]
    fn empty_playbook_produces_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let ansible_dir = dir.path().join("ansible");
        std::fs::create_dir_all(&ansible_dir)?;
        std::fs::write(ansible_dir.join("playbook.yml"), "")?;
        let scanner = AnsibleLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn ignores_yml_files_outside_ansible_dirs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("docker-compose.yml"),
            "version: '3'\nservices:\n  web:\n    image: nginx\n",
        )?;
        let scanner = AnsibleLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }
}
