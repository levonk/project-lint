//! Gradle config scanner — validates `build.gradle` / `settings.gradle` for
//! Gradle/JVM conventions: no dynamic `+` versions, no `SNAPSHOT` versions,
//! `repositories` block presence, `pluginManagement` in settings, wrapper
//! presence, and `gradlew` execute permission.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct GradleConfigScanner {
    forbid_dynamic_versions: bool,
    forbid_snapshots: bool,
    require_wrapper: bool,
    excluded: Vec<String>,
}

impl GradleConfigScanner {
    pub fn new() -> Self {
        Self {
            forbid_dynamic_versions: true,
            forbid_snapshots: true,
            require_wrapper: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        forbid_dynamic_versions: bool,
        forbid_snapshots: bool,
        require_wrapper: bool,
    ) -> Self {
        Self {
            forbid_dynamic_versions,
            forbid_snapshots,
            require_wrapper,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        forbid_dynamic_versions: bool,
        forbid_snapshots: bool,
        require_wrapper: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            forbid_dynamic_versions,
            forbid_snapshots,
            require_wrapper,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();
        let mut has_build_gradle = false;
        let mut has_settings_gradle = false;
        let mut has_wrapper_props = false;
        let mut gradlew_path: Option<std::path::PathBuf> = None;

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if is_excluded_rel(&rel, &self.excluded) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "build.gradle" || name == "build.gradle.kts" {
                has_build_gradle = true;
                issues.extend(self.scan_build_gradle(path, &rel));
            }
            if name == "settings.gradle" || name == "settings.gradle.kts" {
                has_settings_gradle = true;
                issues.extend(self.scan_settings_gradle(path, &rel));
            }
            if name == "gradle-wrapper.properties" {
                has_wrapper_props = true;
            }
            if name == "gradlew" {
                gradlew_path = Some(path.to_path_buf());
            }
        }

        if self.require_wrapper && has_build_gradle && !has_wrapper_props {
            issues.push(ScannerIssue::new(
                "gradle-wrapper-present",
                "warning",
                "gradle/wrapper/gradle-wrapper.properties",
                "build.gradle exists but gradle-wrapper.properties is missing",
            ));
        }

        if has_build_gradle {
            if let Some(gradlew) = &gradlew_path {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = std::fs::metadata(gradlew) {
                        let mode = meta.permissions().mode();
                        if mode & 0o100 == 0 {
                            issues.push(ScannerIssue::new(
                                "gradle-no-gradlew-exec",
                                "warning",
                                "gradlew",
                                "gradlew is not executable — run chmod +x gradlew",
                            ));
                        }
                    }
                }
                let _ = gradlew;
            }
        }

        let _ = has_settings_gradle;
        Ok(issues)
    }

    fn scan_build_gradle(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut has_repositories = false;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            if trimmed.contains("repositories") && trimmed.contains("{") {
                has_repositories = true;
            }

            if self.forbid_dynamic_versions {
                if is_dependency_line(trimmed) && trimmed.contains(":+") {
                    issues.push(
                        ScannerIssue::new(
                            "gradle-no-dynamic-versions",
                            "error",
                            rel,
                            "dependency uses dynamic '+' version — pin to specific version",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if self.forbid_snapshots && is_dependency_line(trimmed) {
                if trimmed.to_uppercase().contains("SNAPSHOT") {
                    issues.push(
                        ScannerIssue::new(
                            "gradle-no-snapshots",
                            "warning",
                            rel,
                            "dependency uses SNAPSHOT version — pin to release version",
                        )
                        .at_line(i + 1),
                    );
                }
            }
        }

        if !has_repositories {
            issues.push(ScannerIssue::new(
                "gradle-repositories-block",
                "info",
                rel,
                "build.gradle missing 'repositories' block",
            ));
        }

        issues
    }

    fn scan_settings_gradle(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        if !content.contains("pluginManagement") {
            issues.push(ScannerIssue::new(
                "gradle-settings-plugin-management",
                "info",
                rel,
                "settings.gradle missing 'pluginManagement' block for plugin resolution",
            ));
        }

        issues
    }
}

fn is_dependency_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("implementation")
        || lower.contains("api ")
        || lower.contains("compile")
        || lower.contains("runtimeonly")
        || lower.contains("dependency")
}

impl Default for GradleConfigScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn valid_build_gradle_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let wrapper_dir = dir.path().join("gradle").join("wrapper");
        std::fs::create_dir_all(&wrapper_dir)?;
        std::fs::write(
            dir.path().join("build.gradle"),
            "repositories {\n    mavenCentral()\n}\n\
             dependencies {\n    implementation 'com.google.guava:guava:32.1.3-jre'\n}\n",
        )?;
        std::fs::write(
            wrapper_dir.join("gradle-wrapper.properties"),
            "distributionUrl=\n",
        )?;
        std::fs::write(
            dir.path().join("settings.gradle"),
            "pluginManagement {\n}\nrootProject.name = 'foo'\n",
        )?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn dynamic_version_flags_error() -> Result<()> {
        let dir = TempDir::new()?;
        let wrapper_dir = dir.path().join("gradle").join("wrapper");
        std::fs::create_dir_all(&wrapper_dir)?;
        std::fs::write(
            dir.path().join("build.gradle"),
            "repositories {\n    mavenCentral()\n}\n\
             dependencies {\n    implementation 'com.google.guava:guava:+'\n}\n",
        )?;
        std::fs::write(wrapper_dir.join("gradle-wrapper.properties"), "")?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "gradle-no-dynamic-versions" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn snapshot_version_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        let wrapper_dir = dir.path().join("gradle").join("wrapper");
        std::fs::create_dir_all(&wrapper_dir)?;
        std::fs::write(
            dir.path().join("build.gradle"),
            "repositories {\n    mavenCentral()\n}\n\
             dependencies {\n    implementation 'com.example:foo:1.0-SNAPSHOT'\n}\n",
        )?;
        std::fs::write(wrapper_dir.join("gradle-wrapper.properties"), "")?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "gradle-no-snapshots"));
        Ok(())
    }

    #[test]
    fn missing_repositories_flags_info() -> Result<()> {
        let dir = TempDir::new()?;
        let wrapper_dir = dir.path().join("gradle").join("wrapper");
        std::fs::create_dir_all(&wrapper_dir)?;
        std::fs::write(
            dir.path().join("build.gradle"),
            "dependencies {\n    implementation 'com.example:foo:1.0'\n}\n",
        )?;
        std::fs::write(wrapper_dir.join("gradle-wrapper.properties"), "")?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "gradle-repositories-block" && i.severity == "info"));
        Ok(())
    }

    #[test]
    fn missing_plugin_management_flags_info() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("settings.gradle"),
            "rootProject.name = 'foo'\n",
        )?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "gradle-settings-plugin-management"));
        Ok(())
    }

    #[test]
    fn missing_wrapper_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("build.gradle"),
            "repositories {\n    mavenCentral()\n}\n",
        )?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "gradle-wrapper-present"));
        Ok(())
    }

    #[test]
    fn no_gradle_files_is_silent() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_build_gradle_flags_repositories() -> Result<()> {
        let dir = TempDir::new()?;
        let wrapper_dir = dir.path().join("gradle").join("wrapper");
        std::fs::create_dir_all(&wrapper_dir)?;
        std::fs::write(dir.path().join("build.gradle"), "")?;
        std::fs::write(wrapper_dir.join("gradle-wrapper.properties"), "")?;
        let scanner = GradleConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "gradle-repositories-block"));
        Ok(())
    }
}
