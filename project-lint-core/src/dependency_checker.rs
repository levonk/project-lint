use crate::utils::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionType {
    Major,
    Minor,
    Patch,
}

#[derive(Debug, Clone)]
pub struct OutdatedDependency {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub version_type: VersionType,
    pub package_manager: String,
    pub file_path: String,
}

pub struct DependencyChecker {
    timeout_seconds: u64,
}

impl DependencyChecker {
    pub fn new() -> Self {
        Self {
            timeout_seconds: 30,
        }
    }

    pub async fn check_dependencies(&self, project_path: &str) -> Result<Vec<OutdatedDependency>> {
        let mut outdated_deps = Vec::new();

        // Check npm packages
        outdated_deps.extend(self.check_npm_packages(project_path).await?);
        // Check cargo dependencies
        outdated_deps.extend(self.check_cargo_dependencies(project_path).await?);

        Ok(outdated_deps)
    }

    async fn check_npm_packages(&self, project_path: &str) -> Result<Vec<OutdatedDependency>> {
        let mut outdated = Vec::new();

        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path.file_name() == Some(std::ffi::OsStr::new("package.json")) {
                if let Ok(deps) = self.check_npm_file(path).await {
                    outdated.extend(deps);
                }
            }
        }

        Ok(outdated)
    }

    async fn check_npm_file(&self, package_json_path: &Path) -> Result<Vec<OutdatedDependency>> {
        let mut outdated = Vec::new();

        // For now, just return empty - we'll implement the actual checking later
        debug!("Checking npm file: {}", package_json_path.display());

        Ok(outdated)
    }

    async fn check_cargo_dependencies(&self, project_path: &str) -> Result<Vec<OutdatedDependency>> {
        let mut outdated = Vec::new();

        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml")) {
                if let Ok(deps) = self.check_cargo_file(path).await {
                    outdated.extend(deps);
                }
            }
        }

        Ok(outdated)
    }

    async fn check_cargo_file(&self, cargo_toml_path: &Path) -> Result<Vec<OutdatedDependency>> {
        let mut outdated = Vec::new();

        // For now, just return empty - we'll implement the actual checking later
        debug!("Checking cargo file: {}", cargo_toml_path.display());

        Ok(outdated)
    }
}

impl Default for DependencyChecker {
    fn default() -> Self {
        Self::new()
    }
}
