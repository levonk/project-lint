use crate::utils::Result;
use crate::dependency_checker::{DependencyChecker, OutdatedDependency};
use colored::Colorize;
use std::path::Path;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

pub struct DependencyVersionChecker {
    checker: DependencyChecker,
}

impl DependencyVersionChecker {
    pub fn new() -> Self {
        Self {
            checker: DependencyChecker::new(),
        }
    }

    pub async fn scan(&self, project_path: &str) -> Result<Vec<DependencyIssue>> {
        debug!("Starting dependency version scan in: {}", project_path);

        let outdated_deps = self.checker.check_dependencies(project_path).await?;
        let mut issues = Vec::new();

        for dep in outdated_deps {
            let severity = match dep.version_type {
                crate::dependency_checker::VersionType::Major => Severity::Error,
                crate::dependency_checker::VersionType::Minor => Severity::Warning,
                crate::dependency_checker::VersionType::Patch => Severity::Info,
            };

            issues.push(DependencyIssue {
                name: dep.name,
                current_version: dep.current_version,
                latest_version: dep.latest_version,
                version_type: dep.version_type,
                package_manager: dep.package_manager,
                file_path: dep.file_path,
                severity,
                message: self.format_issue_message(&dep),
            });
        }

        debug!("Found {} dependency issues", issues.len());
        Ok(issues)
    }

    fn format_issue_message(&self, dep: &OutdatedDependency) -> String {
        let version_type_str = match dep.version_type {
            crate::dependency_checker::VersionType::Major => "MAJOR",
            crate::dependency_checker::VersionType::Minor => "minor",
            crate::dependency_checker::VersionType::Patch => "patch",
        };

        let action = match dep.version_type {
            crate::dependency_checker::VersionType::Major => "is significantly outdated",
            crate::dependency_checker::VersionType::Minor => "has a minor update available",
            crate::dependency_checker::VersionType::Patch => "has a patch update available",
        };

        format!(
            "{} {} {} ({} -> {}) via {}",
            dep.name,
            action,
            format!("({})", version_type_str),
            dep.current_version,
            dep.latest_version,
            dep.package_manager
        )
    }

    pub async fn apply_fixes(&self, issues: &[DependencyIssue], dry_run: bool) -> Result<usize> {
        let mut fixes_applied = 0;

        for issue in issues {
            match self.update_dependency(issue, dry_run).await {
                Ok(success) => {
                    if success {
                        fixes_applied += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to update dependency {}: {}", issue.name, e);
                }
            }
        }

        if fixes_applied > 0 {
            if dry_run {
                info!("📋 Would update {} dependencies", fixes_applied);
            } else {
                info!("✅ Updated {} dependencies", fixes_applied);
            }
        }

        Ok(fixes_applied)
    }

    async fn update_dependency(&self, issue: &DependencyIssue, dry_run: bool) -> Result<bool> {
        let file_path = Path::new(&issue.file_path);

        if !file_path.exists() {
            return Err(anyhow::anyhow!("File not found: {}", issue.file_path));
        }

        // For now, just return false - we'll implement actual updates later
        debug!("Would update {} to {} in {}", issue.name, issue.latest_version, issue.file_path);

        Ok(false)
    }
}

impl Default for DependencyVersionChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct DependencyIssue {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub version_type: crate::dependency_checker::VersionType,
    pub package_manager: String,
    pub file_path: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum Severity {
    Error,
    Warning,
    Info,
}
