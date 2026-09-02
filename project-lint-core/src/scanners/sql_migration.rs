//! SQL migration scanner — validates `*.sql` migration files for naming
//! conventions, numbering gaps, dangerous operations, idempotency, and
//! hardcoded secrets.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::collections::BTreeSet;
use std::path::Path;

pub struct SqlMigrationScanner {
    require_sequential: bool,
    require_idempotent: bool,
    forbid_drop_table: bool,
    forbid_drop_database: bool,
    migration_dirs: Vec<String>,
    excluded: Vec<String>,
}

impl SqlMigrationScanner {
    pub fn new() -> Self {
        Self {
            require_sequential: true,
            require_idempotent: false,
            forbid_drop_table: true,
            forbid_drop_database: true,
            migration_dirs: default_migration_dirs(),
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_sequential: bool,
        require_idempotent: bool,
        forbid_drop_table: bool,
        forbid_drop_database: bool,
        migration_dirs: Vec<String>,
    ) -> Self {
        Self {
            require_sequential,
            require_idempotent,
            forbid_drop_table,
            forbid_drop_database,
            migration_dirs,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_sequential: bool,
        require_idempotent: bool,
        forbid_drop_table: bool,
        forbid_drop_database: bool,
        migration_dirs: Vec<String>,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_sequential,
            require_idempotent,
            forbid_drop_table,
            forbid_drop_database,
            migration_dirs,
            excluded,
        }
    }

    /// Scan a project for SQL migration files and lint each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();
        let mut migration_files: Vec<(std::path::PathBuf, String)> = Vec::new();

        for entry in walk_project(root, &self.excluded, 6).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !name.ends_with(".sql") {
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
            if !self.is_in_migration_dir(&rel) {
                continue;
            }
            migration_files.push((path.to_path_buf(), rel));
        }

        if migration_files.is_empty() {
            return Ok(issues);
        }

        for (path, rel) in &migration_files {
            issues.extend(self.scan_sql_file(path, rel));
        }

        if self.require_sequential {
            issues.extend(self.check_numbering(&migration_files));
        }

        Ok(issues)
    }

    fn is_in_migration_dir(&self, rel: &str) -> bool {
        if self.migration_dirs.is_empty() {
            return true;
        }
        for dir in &self.migration_dirs {
            let dir_norm = dir.trim_matches('/');
            if rel.starts_with(&format!("{}/", dir_norm)) || rel == dir_norm {
                return true;
            }
            let dir_last = dir_norm.rsplit('/').next().unwrap_or(dir_norm);
            if rel.starts_with(&format!("{}/", dir_last)) {
                return true;
            }
        }
        false
    }

    fn scan_sql_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if let Some(issue) = self.check_filename_description(&filename, rel) {
            issues.push(issue);
        }

        let mut has_begin = false;
        let mut has_commit = false;
        let mut has_ddl = false;
        let mut has_if_not_exists = false;

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") || trimmed.starts_with("//") {
                continue;
            }
            let upper = trimmed.to_uppercase();

            if upper.contains("DROP DATABASE") && self.forbid_drop_database {
                issues.push(
                    ScannerIssue::new(
                        "sql-migration-no-drop-database",
                        "error",
                        rel,
                        "DROP DATABASE is forbidden in migrations",
                    )
                    .at_line(i + 1),
                );
            }

            if upper.contains("DROP TABLE") && self.forbid_drop_table {
                if !upper.contains("IF EXISTS") {
                    issues.push(
                        ScannerIssue::new(
                            "sql-migration-no-drop-table",
                            "error",
                            rel,
                            "DROP TABLE without IF EXISTS guard",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if upper.starts_with("BEGIN") {
                has_begin = true;
            }
            if upper.starts_with("COMMIT") {
                has_commit = true;
            }
            if upper.contains("CREATE TABLE")
                || upper.contains("ALTER TABLE")
                || upper.contains("CREATE INDEX")
                || upper.contains("CREATE TYPE")
            {
                has_ddl = true;
            }
            if upper.contains("IF NOT EXISTS") || upper.contains("IF EXISTS") {
                has_if_not_exists = true;
            }

            if upper.contains("CREATE") && upper.contains("VIEW") && upper.contains("SELECT *") {
                issues.push(
                    ScannerIssue::new(
                        "sql-migration-no-select-star",
                        "info",
                        rel,
                        "View uses SELECT * — prefer explicit column lists",
                    )
                    .at_line(i + 1),
                );
            }

            if self.contains_hardcoded_secret(&upper, trimmed) {
                issues.push(
                    ScannerIssue::new(
                        "sql-migration-no-hardcoded-secrets",
                        "error",
                        rel,
                        "Hardcoded password, token, or key in INSERT/statement",
                    )
                    .at_line(i + 1),
                );
            }
        }

        if has_ddl && self.require_idempotent && !has_if_not_exists && !has_begin {
            issues.push(ScannerIssue::new(
                "sql-migration-idempotent",
                "info",
                rel,
                "DDL migration lacks IF NOT EXISTS guards or BEGIN/COMMIT wrapper for idempotency",
            ));
        }

        if has_ddl && has_begin && !has_commit {
            issues.push(ScannerIssue::new(
                "sql-migration-transactional",
                "warning",
                rel,
                "DDL migration has BEGIN but no COMMIT — transaction not closed",
            ));
        }

        issues
    }

    fn check_filename_description(&self, filename: &str, rel: &str) -> Option<ScannerIssue> {
        let stem = filename.strip_suffix(".sql").unwrap_or(filename);
        if let Some(idx) = stem.find('_') {
            let desc = &stem[idx + 1..];
            if desc.is_empty() || desc.chars().all(|c| c.is_numeric()) {
                return Some(ScannerIssue::new(
                    "sql-migration-has-description",
                    "info",
                    rel,
                    "Migration filename lacks a description after the number",
                ));
            }
        } else if stem.chars().all(|c| c.is_numeric()) {
            return Some(ScannerIssue::new(
                "sql-migration-has-description",
                "info",
                rel,
                "Migration filename lacks a description (e.g. 0001_initial_schema.sql)",
            ));
        }
        None
    }

    fn check_numbering(&self, files: &[(std::path::PathBuf, String)]) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let mut by_dir: std::collections::HashMap<String, BTreeSet<u32>> =
            std::collections::HashMap::new();

        for (_path, rel) in files {
            let dir = Path::new(rel)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let filename = Path::new(rel)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Some(num) = extract_migration_number(&filename) {
                by_dir.entry(dir).or_default().insert(num);
            }
        }

        for (dir, nums) in &by_dir {
            let sorted: Vec<u32> = nums.iter().copied().collect();
            if sorted.is_empty() {
                continue;
            }
            let max = *sorted.last().unwrap();
            for n in 1..=max {
                if !nums.contains(&n) {
                    issues.push(ScannerIssue::new(
                        "sql-migration-no-gaps",
                        "warning",
                        dir,
                        format!("Migration numbering gap: missing migration {}", n),
                    ));
                }
            }
        }

        issues
    }

    fn contains_hardcoded_secret(&self, upper: &str, trimmed: &str) -> bool {
        if !upper.contains("INSERT") && !upper.contains("PASSWORD") && !upper.contains("TOKEN") {
            return false;
        }
        let patterns = [
            "PASSWORD '",
            "PASSWORD=\"",
            "TOKEN '",
            "TOKEN=\"",
            "API_KEY '",
            "API_KEY=\"",
            "SECRET '",
            "SECRET=\"",
        ];
        for pat in patterns {
            if upper.contains(pat) {
                let after = upper.split(pat).nth(1).unwrap_or("");
                if let Some(end) = after.find(|c: char| c == '\'' || c == '"') {
                    let val = &after[..end];
                    if val.len() >= 4 && val != "''" && val != "\"\"" {
                        return true;
                    }
                }
            }
        }
        if upper.contains("INSERT") && upper.contains("PASSWORD") {
            if let Some(vals) = upper.split("VALUES").nth(1) {
                if vals.contains('\'') || vals.contains('"') {
                    return true;
                }
            }
        }
        if trimmed.contains("'password_") || trimmed.contains("\"password_") {
            return true;
        }
        false
    }
}

fn default_migration_dirs() -> Vec<String> {
    vec![
        "migrations".to_string(),
        "db/migrations".to_string(),
        "sql/migrations".to_string(),
    ]
}

fn extract_migration_number(filename: &str) -> Option<u32> {
    let stem = filename.strip_suffix(".sql").unwrap_or(filename);
    let prefix = stem.split('_').next().unwrap_or(stem);
    prefix.parse::<u32>().ok()
}

impl Default for SqlMigrationScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clean_migrations_produce_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001_initial_schema.sql"),
            "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY);\n",
        )?;
        std::fs::write(
            mig.join("0002_add_email.sql"),
            "ALTER TABLE users ADD COLUMN email TEXT;\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_drop_table_without_guard() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(mig.join("0001_drop.sql"), "DROP TABLE users;\n")?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-drop-table"));
        Ok(())
    }

    #[test]
    fn allows_drop_table_with_if_exists() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001_drop_safe.sql"),
            "DROP TABLE IF EXISTS old_table;\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-drop-table"));
        Ok(())
    }

    #[test]
    fn flags_drop_database() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(mig.join("0001_drop_db.sql"), "DROP DATABASE production;\n")?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-drop-database"));
        Ok(())
    }

    #[test]
    fn flags_numbering_gap() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001_first.sql"),
            "CREATE TABLE IF NOT EXISTS a (id INT);\n",
        )?;
        std::fs::write(
            mig.join("0003_third.sql"),
            "CREATE TABLE IF NOT EXISTS b (id INT);\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "sql-migration-no-gaps"));
        Ok(())
    }

    #[test]
    fn flags_missing_description() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001.sql"),
            "CREATE TABLE IF NOT EXISTS a (id INT);\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sql-migration-has-description"));
        Ok(())
    }

    #[test]
    fn flags_select_star_in_view() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001_view.sql"),
            "CREATE VIEW v AS SELECT * FROM users;\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-select-star"));
        Ok(())
    }

    #[test]
    fn flags_hardcoded_password_in_insert() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001_seed.sql"),
            "INSERT INTO users (name, password) VALUES ('admin', 'supersecret123');\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-hardcoded-secrets"));
        Ok(())
    }

    #[test]
    fn silent_when_no_sql_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "no sql here")?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn silent_for_sql_outside_migration_dirs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("query.sql"), "DROP TABLE foo;\n")?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_sql_file_produces_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(mig.join("0001_empty.sql"), "")?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn flags_unclosed_transaction() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(
            mig.join("0001_tx.sql"),
            "BEGIN;\nCREATE TABLE foo (id INT);\n",
        )?;
        let scanner = SqlMigrationScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "sql-migration-transactional"));
        Ok(())
    }

    #[test]
    fn config_can_disable_drop_checks() -> Result<()> {
        let dir = TempDir::new()?;
        let mig = dir.path().join("migrations");
        std::fs::create_dir_all(&mig)?;
        std::fs::write(mig.join("0001_drop.sql"), "DROP TABLE users;\n")?;
        let scanner =
            SqlMigrationScanner::with_config(true, false, false, false, default_migration_dirs());
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-drop-table"));
        assert!(!issues
            .iter()
            .any(|i| i.rule == "sql-migration-no-drop-database"));
        Ok(())
    }
}
