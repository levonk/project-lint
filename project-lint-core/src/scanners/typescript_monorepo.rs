//! TypeScript monorepo scanner — verifies pnpm workspace declaration and
//! checks tsconfig path aliases are configured (not bare relative imports).

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;

pub struct TypeScriptMonorepoScanner {
    catalog_mode: bool,
    allowed_extensions: Vec<String>,
}

impl TypeScriptMonorepoScanner {
    pub fn new() -> Self {
        Self {
            catalog_mode: false,
            allowed_extensions: vec![".ts".to_string(), ".tsx".to_string()],
        }
    }

    pub fn with_config(catalog_mode: bool, allowed_extensions: Vec<String>) -> Self {
        Self {
            catalog_mode,
            allowed_extensions,
        }
    }

    /// Scan a TS project root. Only activates when a `tsconfig.json` exists.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        if !root.join("tsconfig.json").exists() {
            return Ok(issues);
        }

        if !root.join("pnpm-workspace.yaml").exists() && !root.join("pnpm-workspace.yml").exists() {
            issues.push(ScannerIssue::new(
                "require-pnpm-workspace",
                "warning",
                "pnpm-workspace.yaml",
                "TypeScript monorepo missing pnpm-workspace.yaml",
            ));
        }

        if let Ok(content) = std::fs::read_to_string(root.join("tsconfig.json")) {
            if !content.contains("\"paths\"") {
                issues.push(ScannerIssue::new(
                    "no-bare-path-aliases",
                    "info",
                    "tsconfig.json",
                    "tsconfig.json has no compilerOptions.paths; bare relative imports likely",
                ));
            }
        }

        if self.catalog_mode {
            let catalog = root.join("pnpm-workspace.yaml");
            if let Ok(content) = std::fs::read_to_string(&catalog) {
                if !content.contains("catalog:") {
                    issues.push(ScannerIssue::new(
                        "require-catalog",
                        "info",
                        "pnpm-workspace.yaml",
                        "catalog mode enabled but no 'catalog:' section in pnpm-workspace.yaml",
                    ));
                }
            }
        }

        Ok(issues)
    }
}

impl Default for TypeScriptMonorepoScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn inactive_without_tsconfig() -> Result<()> {
        let dir = TempDir::new()?;
        let scanner = TypeScriptMonorepoScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_missing_pnpm_workspace_and_paths() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("tsconfig.json"), "{}")?;
        let scanner = TypeScriptMonorepoScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "require-pnpm-workspace"));
        assert!(issues.iter().any(|i| i.rule == "no-bare-path-aliases"));
        Ok(())
    }

    #[test]
    fn clean_ts_monorepo_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions": {"paths": {"@/*": ["src/*"]}}}"#,
        )?;
        std::fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - apps/*\n",
        )?;
        let scanner = TypeScriptMonorepoScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }
}
