//! Prisma schema scanner — validates `*.prisma` files for datasource
//! provider/url, generator client, model @id fields, timestamps, relation
//! indexes, cascade deletes, and hardcoded connection strings.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct PrismaSchemaScanner {
    require_datasource: bool,
    require_generator: bool,
    require_model_timestamps: bool,
    excluded: Vec<String>,
}

impl PrismaSchemaScanner {
    pub fn new() -> Self {
        Self {
            require_datasource: true,
            require_generator: true,
            require_model_timestamps: false,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(
        require_datasource: bool,
        require_generator: bool,
        require_model_timestamps: bool,
    ) -> Self {
        Self {
            require_datasource,
            require_generator,
            require_model_timestamps,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_datasource: bool,
        require_generator: bool,
        require_model_timestamps: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_datasource,
            require_generator,
            require_model_timestamps,
            excluded,
        }
    }

    /// Scan a project for `.prisma` files and lint each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 6).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".prisma") {
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
            issues.extend(self.scan_prisma_file(path, &rel));
        }

        Ok(issues)
    }

    fn scan_prisma_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let blocks = parse_blocks(&content);

        let has_datasource = blocks.iter().any(|b| b.block_type == "datasource");
        let has_generator = blocks.iter().any(|b| b.block_type == "generator");

        if self.require_datasource && !has_datasource {
            issues.push(ScannerIssue::new(
                "prisma-datasource-provider",
                "error",
                rel,
                "Prisma schema missing datasource block",
            ));
        }

        if self.require_generator && !has_generator {
            issues.push(ScannerIssue::new(
                "prisma-generator-client",
                "info",
                rel,
                "Prisma schema missing generator block",
            ));
        }

        for block in &blocks {
            match block.block_type.as_str() {
                "datasource" => issues.extend(self.check_datasource(block, rel)),
                "generator" => issues.extend(self.check_generator(block, rel)),
                "model" => issues.extend(self.check_model(block, rel)),
                _ => {}
            }
        }

        issues.extend(self.check_hardcoded_secrets(&content, rel));

        issues
    }

    fn check_datasource(&self, block: &PrismaBlock, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let has_provider = block.body.lines().any(|l| l.trim().starts_with("provider"));
        if !has_provider {
            issues.push(ScannerIssue::new(
                "prisma-datasource-provider",
                "error",
                rel,
                "datasource block missing provider field",
            ));
        }

        for (i, line) in block.body.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("url") {
                let after_eq = trimmed.split('=').nth(1).unwrap_or("").trim();
                if !after_eq.contains("env(") && !after_eq.contains("env (") {
                    issues.push(
                        ScannerIssue::new(
                            "prisma-datasource-url",
                            "error",
                            rel,
                            "datasource url should use env(\"DATABASE_URL\") not a hardcoded value",
                        )
                        .at_line(block.start_line + i + 1),
                    );
                }
            }
        }

        issues
    }

    fn check_generator(&self, block: &PrismaBlock, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let has_provider = block.body.lines().any(|l| l.trim().starts_with("provider"));
        if !has_provider {
            issues.push(ScannerIssue::new(
                "prisma-generator-client",
                "info",
                rel,
                "generator block missing provider field",
            ));
        }
        issues
    }

    fn check_model(&self, block: &PrismaBlock, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let body_text = block.body.as_str();
        let has_id = body_text.lines().any(|l| l.trim().contains("@id"));
        let has_created_at = body_text
            .lines()
            .any(|l| l.trim().to_lowercase().contains("createdat"));
        let has_updated_at = body_text
            .lines()
            .any(|l| l.trim().to_lowercase().contains("updatedat"));
        let has_relation = body_text.lines().any(|l| l.trim().contains("@relation"));
        let has_index = body_text.lines().any(|l| l.trim().contains("@@index"));
        let has_cascade = body_text
            .lines()
            .any(|l| l.trim().to_lowercase().contains("ondelete: cascade"));

        if !has_id {
            issues.push(ScannerIssue::new(
                "prisma-model-id-field",
                "error",
                rel,
                format!("Model '{}' missing @id field", block.name),
            ));
        }

        if self.require_model_timestamps && (!has_created_at || !has_updated_at) {
            issues.push(ScannerIssue::new(
                "prisma-model-timestamps",
                "info",
                rel,
                format!(
                    "Model '{}' should have createdAt and updatedAt fields",
                    block.name
                ),
            ));
        }

        if has_relation && !has_index {
            issues.push(ScannerIssue::new(
                "prisma-relation-index",
                "warning",
                rel,
                format!(
                    "Model '{}' has @relation but no @@index for foreign key columns",
                    block.name
                ),
            ));
        }

        if has_cascade {
            for (i, line) in block.body.lines().enumerate() {
                if line.trim().to_lowercase().contains("ondelete: cascade") {
                    issues.push(
                        ScannerIssue::new(
                            "prisma-no-cascade-delete",
                            "warning",
                            rel,
                            "Relation uses onDelete: Cascade — use with caution",
                        )
                        .at_line(block.start_line + i + 1),
                    );
                }
            }
        }

        issues
    }

    fn check_hardcoded_secrets(&self, content: &str, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if trimmed.contains("postgresql://")
                || trimmed.contains("mysql://")
                || trimmed.contains("mongodb+srv://")
            {
                if !trimmed.contains("env(") && !trimmed.contains("env (") {
                    issues.push(
                        ScannerIssue::new(
                            "prisma-no-hardcoded-secrets",
                            "error",
                            rel,
                            "Hardcoded database connection string detected",
                        )
                        .at_line(i + 1),
                    );
                }
            }
            let lower = trimmed.to_lowercase();
            if lower.starts_with("password ") || lower.contains(" password ") {
                if lower.contains("= \"") || lower.contains("=\"") {
                    if !lower.contains("env(") {
                        issues.push(
                            ScannerIssue::new(
                                "prisma-no-hardcoded-secrets",
                                "error",
                                rel,
                                "Hardcoded password detected in schema",
                            )
                            .at_line(i + 1),
                        );
                    }
                }
            }
        }
        issues
    }
}

struct PrismaBlock {
    block_type: String,
    name: String,
    body: String,
    start_line: usize,
}

fn parse_blocks(content: &str) -> Vec<PrismaBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if let Some(rest) = trimmed
            .strip_prefix("datasource ")
            .or_else(|| trimmed.strip_prefix("generator "))
            .or_else(|| trimmed.strip_prefix("model "))
            .or_else(|| trimmed.strip_prefix("enum "))
        {
            let block_type = trimmed.split_whitespace().next().unwrap_or("").to_string();
            let name = rest.trim_end_matches('{').trim().to_string();
            let start_line = i;
            let mut brace_depth = 0;
            let mut body = String::new();
            let mut j = i;
            while j < lines.len() {
                let line = lines[j];
                body.push_str(line);
                body.push('\n');
                brace_depth += line.matches('{').count() as i32;
                brace_depth -= line.matches('}').count() as i32;
                if brace_depth <= 0 && j > i {
                    break;
                }
                j += 1;
            }
            blocks.push(PrismaBlock {
                block_type,
                name,
                body,
                start_line,
            });
            i = j + 1;
        } else {
            i += 1;
        }
    }
    blocks
}

impl Default for PrismaSchemaScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn clean_schema() -> &'static str {
        "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id        Int      @id @default(autoincrement())\n  email     String   @unique\n  createdAt DateTime @default(now())\n  updatedAt DateTime @updatedAt\n  posts     Post[]\n  @@index([email])\n}\n\nmodel Post {\n  id        Int      @id @default(autoincrement())\n  authorId  Int\n  author    User     @relation(fields: [authorId], references: [id])\n  @@index([authorId])\n}\n"
    }

    #[test]
    fn clean_schema_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("schema.prisma"), clean_schema())?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_datasource() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "generator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id Int @id\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "prisma-datasource-provider"));
        Ok(())
    }

    #[test]
    fn flags_missing_provider_in_datasource() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id Int @id\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "prisma-datasource-provider"));
        Ok(())
    }

    #[test]
    fn flags_hardcoded_url() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = \"postgresql://localhost:5432/db\"\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id Int @id\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "prisma-datasource-url"));
        assert!(issues
            .iter()
            .any(|i| i.rule == "prisma-no-hardcoded-secrets"));
        Ok(())
    }

    #[test]
    fn flags_missing_generator() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\nmodel User {\n  id Int @id\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "prisma-generator-client"));
        Ok(())
    }

    #[test]
    fn flags_model_without_id() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  email String @unique\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "prisma-model-id-field"));
        Ok(())
    }

    #[test]
    fn flags_relation_without_index() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id       Int   @id\n  posts    Post[]\n}\n\nmodel Post {\n  id       Int  @id\n  authorId Int\n  author   User @relation(fields: [authorId], references: [id])\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "prisma-relation-index"));
        Ok(())
    }

    #[test]
    fn flags_cascade_delete() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id    Int   @id\n  posts Post[]\n}\n\nmodel Post {\n  id       Int  @id\n  authorId Int\n  author   User @relation(fields: [authorId], references: [id], onDelete: Cascade)\n  @@index([authorId])\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "prisma-no-cascade-delete"));
        Ok(())
    }

    #[test]
    fn flags_missing_timestamps_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id Int @id\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::with_config(true, true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "prisma-model-timestamps"));
        Ok(())
    }

    #[test]
    fn silent_when_no_prisma_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "no prisma here")?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_prisma_file_flags_missing_blocks() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("empty.prisma"), "")?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "prisma-datasource-provider"));
        assert!(issues.iter().any(|i| i.rule == "prisma-generator-client"));
        Ok(())
    }

    #[test]
    fn flags_hardcoded_password() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("schema.prisma"),
            "datasource db {\n  provider = \"postgresql\"\n  url = env(\"DATABASE_URL\")\n}\n\ngenerator client {\n  provider = \"prisma-client-js\"\n}\n\nmodel User {\n  id       Int    @id\n  password String = \"mysecret123\"\n}\n",
        )?;
        let scanner = PrismaSchemaScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "prisma-no-hardcoded-secrets"));
        Ok(())
    }
}
