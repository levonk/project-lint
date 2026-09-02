//! Python config scanner — validates `pyproject.toml` for modern Python
//! packaging conventions: build-system presence, `uv`/`ruff` tooling, no `==`
//! pinning, `requires-python` declaration, ruff configuration, and absence of
//! legacy `setup.py` / `requirements.txt`.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct PythonConfigScanner {
    required_tools: Vec<String>,
    forbid_setup_py: bool,
    forbid_requirements_txt: bool,
    excluded: Vec<String>,
}

impl PythonConfigScanner {
    pub fn new() -> Self {
        Self {
            required_tools: vec!["uv".to_string(), "ruff".to_string()],
            forbid_setup_py: true,
            forbid_requirements_txt: false,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        required_tools: Vec<String>,
        forbid_setup_py: bool,
        forbid_requirements_txt: bool,
    ) -> Self {
        Self {
            required_tools,
            forbid_setup_py,
            forbid_requirements_txt,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        required_tools: Vec<String>,
        forbid_setup_py: bool,
        forbid_requirements_txt: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            required_tools,
            forbid_setup_py,
            forbid_requirements_txt,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();
        let mut pyproject_paths: Vec<(std::path::PathBuf, String)> = Vec::new();
        let mut has_setup_py = false;
        let mut has_requirements_txt = false;

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
            if name == "pyproject.toml" {
                pyproject_paths.push((path.to_path_buf(), rel));
            }
            if self.forbid_setup_py && name == "setup.py" {
                has_setup_py = true;
            }
            if self.forbid_requirements_txt && name == "requirements.txt" {
                has_requirements_txt = true;
            }
        }

        for (path, rel) in &pyproject_paths {
            issues.extend(self.scan_pyproject_toml(path, rel));
        }

        if !pyproject_paths.is_empty() && has_setup_py {
            issues.push(ScannerIssue::new(
                "pyproject-no-setup-py",
                "warning",
                "setup.py",
                "setup.py present alongside pyproject.toml — use pyproject.toml only",
            ));
        }

        if !pyproject_paths.is_empty() && has_requirements_txt {
            let has_project_deps = pyproject_paths.iter().any(|(p, _)| {
                std::fs::read_to_string(p)
                    .map(|c| c.contains("[project.dependencies]"))
                    .unwrap_or(false)
            });
            if has_project_deps {
                issues.push(ScannerIssue::new(
                    "pyproject-no-requirements-txt",
                    "info",
                    "requirements.txt",
                    "requirements.txt present with [project.dependencies] — use uv.lock instead",
                ));
            }
        }

        Ok(issues)
    }

    fn scan_pyproject_toml(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        if !content.contains("[build-system]") {
            issues.push(ScannerIssue::new(
                "pyproject-build-system",
                "error",
                rel,
                "pyproject.toml missing [build-system] section with requires and build-backend",
            ));
        } else {
            if !content.contains("requires") {
                issues.push(ScannerIssue::new(
                    "pyproject-build-system",
                    "error",
                    rel,
                    "[build-system] missing 'requires' field",
                ));
            }
            if !content.contains("build-backend") {
                issues.push(ScannerIssue::new(
                    "pyproject-build-system",
                    "error",
                    rel,
                    "[build-system] missing 'build-backend' field",
                ));
            }
        }

        if !self.required_tools.is_empty() {
            let content_lower = content.to_lowercase();
            for tool in &self.required_tools {
                let table = format!("[tool.{}]", tool);
                if !content_lower.contains(&table.to_lowercase()) {
                    issues.push(ScannerIssue::new(
                        "pyproject-uses-uv-or-ruff",
                        "warning",
                        rel,
                        format!(
                            "pyproject.toml should use '{}' (add [tool.{}] section)",
                            tool, tool
                        ),
                    ));
                }
            }
        }

        if content.contains("[project.dependencies]") {
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                if trimmed.contains("==") && !trimmed.contains("===") {
                    issues.push(
                        ScannerIssue::new(
                            "pyproject-no-pinned-equals",
                            "warning",
                            rel,
                            "dependency uses '==' pinning — prefer '>=' ranges or lock file",
                        )
                        .at_line(i + 1),
                    );
                }
            }
        }

        if !content.contains("requires-python") {
            issues.push(ScannerIssue::new(
                "pyproject-python-version",
                "warning",
                rel,
                "[project] missing 'requires-python' field",
            ));
        }

        if content.contains("[tool.ruff]") {
            if !content.contains("line-length") || !content.contains("target-version") {
                issues.push(ScannerIssue::new(
                    "pyproject-ruff-config",
                    "info",
                    rel,
                    "[tool.ruff] should configure 'line-length' and 'target-version'",
                ));
            }
        }

        issues
    }
}

impl Default for PythonConfigScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn valid_pyproject_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\nrequires-python = \">=3.11\"\n\n\
             [project.dependencies]\nbar = \">=1.0\"\n\n\
             [tool.uv]\n[tool.ruff]\nline-length = 100\ntarget-version = \"py311\"\n",
        )?;
        let scanner = PythonConfigScanner::with_config(vec![], false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn missing_build_system_flags_error() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"foo\"\n",
        )?;
        let scanner = PythonConfigScanner::with_config(vec![], false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pyproject-build-system" && i.severity == "error"));
        Ok(())
    }

    #[test]
    fn missing_required_tool_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\nrequires-python = \">=3.11\"\n",
        )?;
        let scanner = PythonConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pyproject-uses-uv-or-ruff"));
        Ok(())
    }

    #[test]
    fn equals_pinning_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\nrequires-python = \">=3.11\"\n\n\
             [project.dependencies]\nbar = \"==1.0\"\n",
        )?;
        let scanner = PythonConfigScanner::with_config(vec![], false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pyproject-no-pinned-equals"));
        Ok(())
    }

    #[test]
    fn missing_requires_python_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\n",
        )?;
        let scanner = PythonConfigScanner::with_config(vec![], false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pyproject-python-version"));
        Ok(())
    }

    #[test]
    fn ruff_without_config_flags_info() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\nrequires-python = \">=3.11\"\n\n\
             [tool.ruff]\n",
        )?;
        let scanner = PythonConfigScanner::with_config(vec![], false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pyproject-ruff-config" && i.severity == "info"));
        Ok(())
    }

    #[test]
    fn setup_py_alongside_pyproject_flags_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\nrequires-python = \">=3.11\"\n",
        )?;
        std::fs::write(
            dir.path().join("setup.py"),
            "from setuptools import setup\nsetup()\n",
        )?;
        let scanner = PythonConfigScanner::with_config(vec![], true, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pyproject-no-setup-py"));
        Ok(())
    }

    #[test]
    fn requirements_txt_with_project_deps_flags_info() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n\
             [project]\nname = \"foo\"\nrequires-python = \">=3.11\"\n\n\
             [project.dependencies]\nbar = \">=1.0\"\n",
        )?;
        std::fs::write(dir.path().join("requirements.txt"), "bar>=1.0\n")?;
        let scanner = PythonConfigScanner::with_config(vec![], false, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "pyproject-no-requirements-txt"));
        Ok(())
    }

    #[test]
    fn no_pyproject_is_silent() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# project\n")?;
        let scanner = PythonConfigScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_pyproject_flags_build_system() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("pyproject.toml"), "")?;
        let scanner = PythonConfigScanner::with_config(vec![], false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "pyproject-build-system"));
        Ok(())
    }
}
