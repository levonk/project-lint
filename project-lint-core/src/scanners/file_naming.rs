use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tracing::{debug, warn, info};
use walkdir::WalkDir;
use colored::Colorize;
use crate::utils::Result;

#[derive(Debug, Clone)]
pub struct NamingIssue {
    pub path: PathBuf,
    pub suggested_name: String,
    pub message: String,
    pub severity: String,
    pub is_directory: bool,
}

pub struct FileNamingScanner {
    /// Map of "wrong/typo name" -> "correct name" for common mistakes
    exact_mismatches: HashMap<String, String>,
    /// List of "correct names" to check against for fuzzy matching
    expected_names: Vec<String>,
}

impl FileNamingScanner {
    pub fn new() -> Self {
        let mut exact_mismatches = HashMap::new();

        // Common plural/singular mistakes
        exact_mismatches.insert(".devcontainers".to_string(), ".devcontainer".to_string());
        exact_mismatches.insert(".githubs".to_string(), ".github".to_string());
        exact_mismatches.insert("packages.json".to_string(), "package.json".to_string());

        // Common extension mistakes
        exact_mismatches.insert("package.jsn".to_string(), "package.json".to_string());
        exact_mismatches.insert("docker-compose.yaml".to_string(), "docker-compose.yml".to_string());
        exact_mismatches.insert("Cargo.toml.lock".to_string(), "Cargo.lock".to_string());

        let expected_names = vec![
            ".devcontainer".to_string(),
            ".github".to_string(),
            ".vscode".to_string(),
            ".gitignore".to_string(),
            ".editorconfig".to_string(),
            "package.json".to_string(),
            "docker-compose.yml".to_string(),
            "Cargo.toml".to_string(),
            "README.md".to_string(),
            "LICENSE".to_string(),
            "Makefile".to_string(),
            "flake.nix".to_string(),
            "devbox.json".to_string(),
        ];

        Self {
            exact_mismatches,
            expected_names,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<NamingIssue>> {
        let mut issues = Vec::new();
        let project_root = Path::new(project_path);

        for entry in WalkDir::new(project_root)
            .max_depth(3) // Mostly focus on root and near-root files
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let is_dir = path.is_dir();

            // Skip the project root itself
            if path == project_root {
                continue;
            }

            // 1. Check exact mismatches (plural/singular, common typos)
            if let Some(correct_name) = self.exact_mismatches.get(&file_name) {
                issues.push(NamingIssue {
                    path: path.to_path_buf(),
                    suggested_name: correct_name.clone(),
                    message: format!(
                        "Found '{}' which should likely be '{}'",
                        file_name.yellow(),
                        correct_name.green()
                    ),
                    severity: "warning".to_string(),
                    is_directory: is_dir,
                });
                continue;
            }

            // 2. Fuzzy matching for typos
            for expected in &self.expected_names {
                if file_name == *expected {
                    continue;
                }

                let distance = levenshtein_distance(&file_name.to_lowercase(), &expected.to_lowercase());

                // If it's very close (1 or 2 chars off) but not exact
                let threshold = if expected.len() > 6 { 2 } else { 1 };

                if distance > 0 && distance <= threshold {
                    // Check if it's already in exact mismatches to avoid double reporting
                    if self.exact_mismatches.contains_key(&file_name) {
                        continue;
                    }

                    issues.push(NamingIssue {
                        path: path.to_path_buf(),
                        suggested_name: expected.clone(),
                        message: format!(
                            "Found '{}' which looks like a typo of '{}' (fuzzy match)",
                            file_name.yellow(),
                            expected.green()
                        ),
                        severity: "warning".to_string(),
                        is_directory: is_dir,
                    });
                }
            }
        }

        Ok(issues)
    }

    pub fn apply_fixes(&self, issues: &[NamingIssue], dry_run: bool) -> Result<usize> {
        let mut fixed_count = 0;

        for issue in issues {
            let old_path = &issue.path;
            let mut new_path = old_path.clone();
            new_path.set_file_name(&issue.suggested_name);

            if dry_run {
                info!("(Dry-run) Would rename {} to {}", old_path.display(), new_path.display());
                fixed_count += 1;
            } else {
                match std::fs::rename(old_path, &new_path) {
                    Ok(_) => {
                        info!("Renamed {} to {}", old_path.display(), new_path.display());
                        fixed_count += 1;
                    }
                    Err(e) => {
                        warn!("Failed to rename {} to {}: {}", old_path.display(), new_path.display(), e);
                    }
                }
            }
        }

        Ok(fixed_count)
    }
}

/// Simple Levenshtein distance implementation
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let n = v1.len();
    let m = v2.len();

    if n == 0 { return m; }
    if m == 0 { return n; }

    let mut dp = vec![vec![0; m + 1]; n + 1];

    for i in 0..=n { dp[i][0] = i; }
    for j in 0..=m { dp[0][j] = j; }

    for i in 1..=n {
        for j in 1..=m {
            let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
            dp[i][j] = std::cmp::min(
                dp[i - 1][j] + 1,
                std::cmp::min(dp[i][j - 1] + 1, dp[i - 1][j - 1] + cost)
            );
        }
    }

    dp[n][m]
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("book", "back"), 2);
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance(".devcontainers", ".devcontainer"), 1);
    }

    #[test]
    fn test_scan_exact_mismatch() -> Result<()> {
        let temp = tempdir()?;
        let project_path = temp.path();

        // Create a directory with a mistake
        let wrong_dir = project_path.join(".devcontainers");
        std::fs::create_dir(&wrong_dir)?;

        let scanner = FileNamingScanner::new();
        let issues = scanner.scan(&project_path.to_string_lossy())?;

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].suggested_name, ".devcontainer");
        assert!(issues[0].is_directory);

        Ok(())
    }

    #[test]
    fn test_scan_fuzzy_match() -> Result<()> {
        let temp = tempdir()?;
        let project_path = temp.path();

        // Create a file with a typo
        let wrong_file = project_path.join("package.jsn");
        std::fs::File::create(&wrong_file)?;

        let scanner = FileNamingScanner::new();
        let issues = scanner.scan(&project_path.to_string_lossy())?;

        // "package.jsn" might trigger both exact mismatch and fuzzy match if not handled,
        // but we have it in exact_mismatches first.
        assert!(issues.iter().any(|i| i.suggested_name == "package.json"));

        Ok(())
    }

    #[test]
    fn test_apply_fixes() -> Result<()> {
        let temp = tempdir()?;
        let project_path = temp.path();

        let wrong_dir = project_path.join(".devcontainers");
        std::fs::create_dir(&wrong_dir)?;

        let scanner = FileNamingScanner::new();
        let issues = scanner.scan(&project_path.to_string_lossy())?;

        let fixed = scanner.apply_fixes(&issues, false)?;
        assert_eq!(fixed, 1);

        assert!(project_path.join(".devcontainer").exists());
        assert!(!project_path.join(".devcontainers").exists());

        Ok(())
    }
}
