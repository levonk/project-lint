//! Compose file lint scanner — enforces container hardening best practices
//! for `docker-compose*.yml` / `compose*.yml` files: pinned image digests,
//! no `:latest` tags, no privileged mode, `security_opt`, no docker.sock
//! mounts, non-root user, no host network/pid, `cap_drop`, `read_only`,
//! healthchecks, restart policies, resource limits, safe port bindings,
//! and (in ops mode) watchtower/wud update labels.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

pub struct ComposeLintScanner {
    require_pinned_digests: bool,
    require_healthcheck: bool,
    require_resource_limits: bool,
    require_no_new_privileges: bool,
    forbid_privileged: bool,
    forbid_docker_sock: bool,
    ops_mode: bool,
    exempt_proxy_labels: Vec<String>,
    excluded: Vec<String>,
}

impl ComposeLintScanner {
    pub fn new() -> Self {
        Self {
            require_pinned_digests: true,
            require_healthcheck: true,
            require_resource_limits: false,
            require_no_new_privileges: true,
            forbid_privileged: true,
            forbid_docker_sock: true,
            ops_mode: false,
            exempt_proxy_labels: vec!["com.dockerproxy.role".to_string()],
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_pinned_digests: bool,
        require_healthcheck: bool,
        require_resource_limits: bool,
        require_no_new_privileges: bool,
        forbid_privileged: bool,
        forbid_docker_sock: bool,
        ops_mode: bool,
        exempt_proxy_labels: Vec<String>,
    ) -> Self {
        Self {
            require_pinned_digests,
            require_healthcheck,
            require_resource_limits,
            require_no_new_privileges,
            forbid_privileged,
            forbid_docker_sock,
            ops_mode,
            exempt_proxy_labels,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_pinned_digests: bool,
        require_healthcheck: bool,
        require_resource_limits: bool,
        require_no_new_privileges: bool,
        forbid_privileged: bool,
        forbid_docker_sock: bool,
        ops_mode: bool,
        exempt_proxy_labels: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_pinned_digests,
            require_healthcheck,
            require_resource_limits,
            require_no_new_privileges,
            forbid_privileged,
            forbid_docker_sock,
            ops_mode,
            exempt_proxy_labels,
            excluded,
        }
    }

    /// Scan a project for compose files and lint each service.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !is_compose_file(&name) {
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
            debug!("compose_lint: scanning {}", rel);
            issues.extend(self.scan_compose_file(path, &rel));
        }

        Ok(issues)
    }

    fn scan_compose_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let parsed: ComposeFile = match serde_yaml::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                return vec![ScannerIssue::new(
                    "compose-parse-error",
                    "error",
                    rel,
                    format!("failed to parse compose file: {}", e),
                )]
            }
        };
        let Some(services) = parsed.services else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        for (service_name, service) in services {
            issues.extend(self.check_service(&service_name, &service, rel));
        }
        issues
    }

    fn check_service(&self, name: &str, svc: &ComposeService, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let loc = format!("{}:{}", rel, name);

        if let Some(image) = &svc.image {
            if self.require_pinned_digests && !image.contains("@sha256:") {
                if image.contains(":latest") {
                    issues.push(ScannerIssue::new(
                        "compose-no-latest-tag",
                        "error",
                        &loc,
                        format!("service '{}' uses ':latest' image tag", name),
                    ));
                } else if is_floating_tag(image) {
                    issues.push(ScannerIssue::new(
                        "compose-no-floating-tag",
                        "warning",
                        &loc,
                        format!(
                            "service '{}' uses floating tag '{}' without digest pinning",
                            name, image
                        ),
                    ));
                } else {
                    issues.push(ScannerIssue::new(
                        "compose-pinned-images",
                        "error",
                        &loc,
                        format!("service '{}' image '{}' not pinned by digest", name, image),
                    ));
                }
            } else if image.contains(":latest") && self.require_pinned_digests {
                // :latest with digest is still :latest
                issues.push(ScannerIssue::new(
                    "compose-no-latest-tag",
                    "error",
                    &loc,
                    format!("service '{}' uses ':latest' image tag", name),
                ));
            }
        }

        if self.forbid_privileged && svc.privileged.unwrap_or(false) {
            issues.push(ScannerIssue::new(
                "compose-no-privileged",
                "error",
                &loc,
                format!("service '{}' runs in privileged mode", name),
            ));
        }

        if self.require_no_new_privileges {
            let has_nnp = svc
                .security_opt
                .as_ref()
                .map(|opts| opts.iter().any(|o| o == "no-new-privileges:true"))
                .unwrap_or(false);
            if !has_nnp {
                issues.push(ScannerIssue::new(
                    "compose-security-opt",
                    "warning",
                    &loc,
                    format!(
                        "service '{}' should set security_opt: [\"no-new-privileges:true\"]",
                        name
                    ),
                ));
            }
        }

        if self.forbid_docker_sock {
            if let Some(volumes) = &svc.volumes {
                let has_sock = volumes.iter().any(|v| {
                    let source = v.split(':').next().unwrap_or(v);
                    source.contains("/var/run/docker.sock")
                });
                if has_sock {
                    let exempt = svc
                        .labels
                        .as_ref()
                        .map(|labels| {
                            self.exempt_proxy_labels
                                .iter()
                                .any(|label| labels.contains_key(label))
                        })
                        .unwrap_or(false);
                    if !exempt {
                        issues.push(ScannerIssue::new(
                            "compose-no-docker-sock-mount",
                            "error",
                            &loc,
                            format!(
                                "service '{}' mounts /var/run/docker.sock without exempt proxy label",
                                name
                            ),
                        ));
                    }
                }
            }
        }

        if svc.user.is_none() {
            issues.push(ScannerIssue::new(
                "compose-no-root-user",
                "warning",
                &loc,
                format!("service '{}' does not specify a non-root user", name),
            ));
        }

        if let Some(net) = &svc.network_mode {
            if net == "host" {
                issues.push(ScannerIssue::new(
                    "compose-no-host-network",
                    "warning",
                    &loc,
                    format!("service '{}' uses network_mode: host", name),
                ));
            }
        }

        if let Some(pid) = &svc.pid {
            if pid == "host" {
                issues.push(ScannerIssue::new(
                    "compose-no-host-pid",
                    "warning",
                    &loc,
                    format!("service '{}' uses pid: host", name),
                ));
            }
        }

        let has_cap_drop_all = svc
            .cap_drop
            .as_ref()
            .map(|caps| caps.iter().any(|c| c == "ALL"))
            .unwrap_or(false);
        if !has_cap_drop_all {
            issues.push(ScannerIssue::new(
                "compose-cap-drop",
                "warning",
                &loc,
                format!("service '{}' should set cap_drop: [\"ALL\"]", name),
            ));
        }

        if !svc.read_only.unwrap_or(false) {
            issues.push(ScannerIssue::new(
                "compose-readonly-filesystem",
                "info",
                &loc,
                format!("service '{}' should set read_only: true", name),
            ));
        }

        if self.require_healthcheck && svc.healthcheck.is_none() {
            issues.push(ScannerIssue::new(
                "compose-healthcheck",
                "warning",
                &loc,
                format!("service '{}' missing healthcheck", name),
            ));
        }

        if let Some(restart) = &svc.restart {
            if restart != "unless-stopped" && restart != "always" {
                issues.push(ScannerIssue::new(
                    "compose-restart-policy",
                    "info",
                    &loc,
                    format!(
                        "service '{}' restart policy '{}' should be 'unless-stopped' or 'always'",
                        name, restart
                    ),
                ));
            }
        } else {
            issues.push(ScannerIssue::new(
                "compose-restart-policy",
                "info",
                &loc,
                format!("service '{}' missing restart policy", name),
            ));
        }

        if self.require_resource_limits {
            let has_limits = svc
                .deploy
                .as_ref()
                .and_then(|d| d.resources.as_ref())
                .and_then(|r| r.limits.as_ref())
                .is_some();
            if !has_limits {
                issues.push(ScannerIssue::new(
                    "compose-resource-limits",
                    "warning",
                    &loc,
                    format!("service '{}' should define deploy.resources.limits", name),
                ));
            }
        }

        if let Some(ports) = &svc.ports {
            for port in ports {
                let binding = port.split(':').next().unwrap_or(port);
                if binding == "0.0.0.0" || binding.is_empty() {
                    issues.push(ScannerIssue::new(
                        "compose-no-bind-0.0.0.0",
                        "warning",
                        &loc,
                        format!(
                            "service '{}' port '{}' binds to 0.0.0.0; use 127.0.0.1 for local-only",
                            name, port
                        ),
                    ));
                }
            }
        }

        if self.ops_mode {
            let has_watchtower = svc
                .labels
                .as_ref()
                .map(|labels| labels.contains_key("com.centurylinklabs.watchtower.enable"))
                .unwrap_or(false);
            let has_wud = svc
                .labels
                .as_ref()
                .map(|labels| labels.contains_key("wud.tag.include"))
                .unwrap_or(false);
            if !has_watchtower && !has_wud {
                issues.push(ScannerIssue::new(
                    "compose-watchtower-labels",
                    "info",
                    &loc,
                    format!(
                        "service '{}' missing watchtower/wud update labels (ops_mode enabled)",
                        name
                    ),
                ));
            }
        }

        issues
    }
}

impl Default for ComposeLintScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn is_compose_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    if !lower.ends_with(".yml") && !lower.ends_with(".yaml") {
        return false;
    }
    lower.starts_with("docker-compose") || lower.starts_with("compose")
}

fn is_floating_tag(image: &str) -> bool {
    if image.contains("@sha256:") {
        return false;
    }
    if let Some(tag_part) = image.rsplit('/').next() {
        if let Some(colon) = tag_part.rfind(':') {
            let tag = &tag_part[colon + 1..];
            return tag.parse::<u64>().is_ok();
        }
    }
    false
}

#[derive(Debug, Deserialize)]
struct ComposeFile {
    services: Option<HashMap<String, ComposeService>>,
}

#[derive(Debug, Deserialize)]
struct ComposeService {
    image: Option<String>,
    privileged: Option<bool>,
    security_opt: Option<Vec<String>>,
    volumes: Option<Vec<String>>,
    user: Option<String>,
    network_mode: Option<String>,
    pid: Option<String>,
    cap_drop: Option<Vec<String>>,
    read_only: Option<bool>,
    healthcheck: Option<ComposeHealthcheck>,
    restart: Option<String>,
    deploy: Option<ComposeDeploy>,
    ports: Option<Vec<String>>,
    labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct ComposeHealthcheck {
    #[serde(default)]
    test: Option<serde_yaml::Value>,
    #[serde(default)]
    disable: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ComposeDeploy {
    resources: Option<ComposeResources>,
}

#[derive(Debug, Deserialize)]
struct ComposeResources {
    limits: Option<serde_yaml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_compose(dir: &Path, content: &str) {
        std::fs::write(dir.join("docker-compose.yml"), content).unwrap();
    }

    #[test]
    fn valid_compose_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc123
    user: "1000:1000"
    security_opt:
      - "no-new-privileges:true"
    cap_drop:
      - "ALL"
    read_only: true
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost"]
    restart: unless-stopped
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"compose-pinned-images"),
            "should not flag pinned image: {:?}",
            rules
        );
        assert!(
            !rules.contains(&"compose-no-root-user"),
            "should not flag user set: {:?}",
            rules
        );
        assert!(
            !rules.contains(&"compose-security-opt"),
            "should not flag security_opt set: {:?}",
            rules
        );
        assert!(
            !rules.contains(&"compose-cap-drop"),
            "should not flag cap_drop set: {:?}",
            rules
        );
        assert!(
            !rules.contains(&"compose-readonly-filesystem"),
            "should not flag read_only set: {:?}",
            rules
        );
        assert!(
            !rules.contains(&"compose-healthcheck"),
            "should not flag healthcheck set: {:?}",
            rules
        );
        assert!(
            !rules.contains(&"compose-restart-policy"),
            "should not flag restart set: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn flags_privileged_and_missing_digest() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25
    privileged: true
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-pinned-images"));
        assert!(rules.contains(&"compose-no-privileged"));
        Ok(())
    }

    #[test]
    fn flags_latest_tag() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:latest
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-no-latest-tag"));
        Ok(())
    }

    #[test]
    fn flags_floating_tag() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-no-floating-tag"));
        Ok(())
    }

    #[test]
    fn flags_docker_sock_mount() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
    volumes:
      - "/var/run/docker.sock:/var/run/docker.sock"
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-no-docker-sock-mount"));
        Ok(())
    }

    #[test]
    fn docker_sock_exempt_by_proxy_label() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  proxy:
    image: nginx:1.25@sha256:abc
    volumes:
      - "/var/run/docker.sock:/var/run/docker.sock"
    labels:
      com.dockerproxy.role: "proxy"
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"compose-no-docker-sock-mount"),
            "proxy label should exempt: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn flags_host_network_and_pid() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
    network_mode: host
    pid: host
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-no-host-network"));
        assert!(rules.contains(&"compose-no-host-pid"));
        Ok(())
    }

    #[test]
    fn flags_bind_0_0_0_0() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
    ports:
      - "0.0.0.0:8080:8080"
"#,
        );
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-no-bind-0.0.0.0"));
        Ok(())
    }

    #[test]
    fn ops_mode_flags_missing_watchtower_labels() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
"#,
        );
        let scanner = ComposeLintScanner::with_config(
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            vec!["com.dockerproxy.role".to_string()],
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-watchtower-labels"));
        Ok(())
    }

    #[test]
    fn ops_mode_silent_when_labels_present() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
    labels:
      com.centurylinklabs.watchtower.enable: "true"
"#,
        );
        let scanner = ComposeLintScanner::with_config(
            true,
            true,
            false,
            true,
            true,
            true,
            true,
            vec!["com.dockerproxy.role".to_string()],
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"compose-watchtower-labels"),
            "watchtower label should satisfy: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn resource_limits_flagged_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
"#,
        );
        let scanner = ComposeLintScanner::with_config(
            true,
            true,
            true,
            true,
            true,
            true,
            false,
            vec!["com.dockerproxy.role".to_string()],
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(rules.contains(&"compose-resource-limits"));
        Ok(())
    }

    #[test]
    fn resource_limits_satisfied() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(
            dir.path(),
            r#"services:
  web:
    image: nginx:1.25@sha256:abc
    deploy:
      resources:
        limits:
          memory: 512M
          cpus: "0.5"
"#,
        );
        let scanner = ComposeLintScanner::with_config(
            true,
            true,
            true,
            true,
            true,
            true,
            false,
            vec!["com.dockerproxy.role".to_string()],
        );
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let rules: Vec<&str> = issues.iter().map(|i| i.rule.as_str()).collect();
        assert!(
            !rules.contains(&"compose-resource-limits"),
            "deploy.resources.limits should satisfy: {:?}",
            rules
        );
        Ok(())
    }

    #[test]
    fn empty_compose_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(dir.path(), "");
        let scanner = ComposeLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn malformed_compose_emits_parse_error() -> Result<()> {
        let dir = TempDir::new()?;
        write_compose(dir.path(), "services: [invalid yaml {{{");
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "compose-parse-error"));
        Ok(())
    }

    #[test]
    fn no_compose_files_silent() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# no compose here")?;
        let scanner = ComposeLintScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn compose_yml_variant_detected() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("compose.yml"),
            r#"services:
  web:
    image: nginx:latest
"#,
        )?;
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "compose-no-latest-tag"));
        Ok(())
    }

    #[test]
    fn compose_override_variant_detected() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("docker-compose.override.yml"),
            r#"services:
  web:
    image: nginx:latest
"#,
        )?;
        let scanner = ComposeLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "compose-no-latest-tag"));
        Ok(())
    }
}
