//! Protobuf lint scanner — validates `*.proto` files for syntax version,
//! package declaration, field/message naming, enum zero values, reserved
//! collisions, unused imports, and deprecated fields without reserved numbers.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct ProtobufLintScanner {
    require_proto3: bool,
    require_enum_zero_value: bool,
    excluded: Vec<String>,
}

impl ProtobufLintScanner {
    pub fn new() -> Self {
        Self {
            require_proto3: true,
            require_enum_zero_value: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_proto3: bool, require_enum_zero_value: bool) -> Self {
        Self {
            require_proto3,
            require_enum_zero_value,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_proto3: bool,
        require_enum_zero_value: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_proto3,
            require_enum_zero_value,
            excluded,
        }
    }

    /// Scan a project for `.proto` files and lint each.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 6).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".proto") {
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
            issues.extend(self.scan_proto_file(path, &rel));
        }

        Ok(issues)
    }

    fn scan_proto_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let mut has_syntax = false;
        let mut has_package = false;
        let mut imports: Vec<String> = Vec::new();
        let mut reserved_numbers: Vec<u32> = Vec::new();
        let mut field_numbers: Vec<(u32, usize)> = Vec::new();
        let mut deprecated_fields: Vec<(u32, usize)> = Vec::new();

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            if trimmed.starts_with("syntax") {
                has_syntax = true;
                if self.require_proto3 && !trimmed.contains("proto3") {
                    issues.push(
                        ScannerIssue::new(
                            "proto-syntax-version",
                            "warning",
                            rel,
                            "Proto file should use syntax = \"proto3\"",
                        )
                        .at_line(i + 1),
                    );
                }
            }

            if trimmed.starts_with("package ") {
                has_package = true;
            }

            if trimmed.starts_with("import ") {
                if let Some(imp) = extract_import(trimmed) {
                    imports.push(imp);
                }
            }

            if let Some(nums) = extract_reserved_numbers(trimmed) {
                reserved_numbers.extend(nums);
            }

            if let Some((num, is_deprecated)) = extract_field_number(trimmed) {
                field_numbers.push((num, i + 1));
                if is_deprecated {
                    deprecated_fields.push((num, i + 1));
                }
            }

            if let Some(issue) = self.check_field_naming(trimmed, rel, i + 1) {
                issues.push(issue);
            }

            if let Some(issue) = self.check_message_naming(trimmed, rel, i + 1) {
                issues.push(issue);
            }

            if let Some(issue) = self.check_enum_zero_value(trimmed, rel, i + 1) {
                issues.push(issue);
            }
        }

        if !has_syntax {
            issues.push(ScannerIssue::new(
                "proto-syntax-version",
                "warning",
                rel,
                "Proto file missing syntax declaration (should be \"proto3\")",
            ));
        }

        if !has_package {
            issues.push(ScannerIssue::new(
                "proto-package-present",
                "error",
                rel,
                "Proto file missing package declaration",
            ));
        }

        for (num, line) in &field_numbers {
            if reserved_numbers.contains(num) {
                issues.push(
                    ScannerIssue::new(
                        "proto-no-reserved-collision",
                        "error",
                        rel,
                        format!("Field number {} collides with a reserved number", num),
                    )
                    .at_line(*line),
                );
            }
        }

        for (num, line) in &deprecated_fields {
            if !reserved_numbers.contains(num) {
                issues.push(
                    ScannerIssue::new(
                        "proto-no-deprecated-fields",
                        "warning",
                        rel,
                        format!(
                            "Deprecated field {} lacks a reserved number for the field",
                            num
                        ),
                    )
                    .at_line(*line),
                );
            }
        }

        if !imports.is_empty() {
            let import_refs: Vec<&str> = imports.iter().map(|s| s.as_str()).collect();
            let unused = find_unused_imports(&import_refs, &content);
            for imp in unused {
                issues.push(ScannerIssue::new(
                    "proto-imports-used",
                    "info",
                    rel,
                    format!("Unused import: {}", imp),
                ));
            }
        }

        issues
    }

    fn check_field_naming(&self, trimmed: &str, rel: &str, line: usize) -> Option<ScannerIssue> {
        if !trimmed.starts_with("repeated ") && !trimmed.starts_with("optional ") {
            if !is_field_line(trimmed) {
                return None;
            }
        }
        if let Some(name) = extract_field_name(trimmed) {
            if !is_snake_case(&name) {
                return Some(
                    ScannerIssue::new(
                        "proto-field-naming",
                        "warning",
                        rel,
                        format!("Field '{}' should use snake_case", name),
                    )
                    .at_line(line),
                );
            }
        }
        None
    }

    fn check_message_naming(&self, trimmed: &str, rel: &str, line: usize) -> Option<ScannerIssue> {
        if let Some(name) = extract_message_name(trimmed) {
            if !is_pascal_case(&name) {
                return Some(
                    ScannerIssue::new(
                        "proto-message-naming",
                        "warning",
                        rel,
                        format!("Message '{}' should use PascalCase", name),
                    )
                    .at_line(line),
                );
            }
        }
        None
    }

    fn check_enum_zero_value(&self, trimmed: &str, rel: &str, line: usize) -> Option<ScannerIssue> {
        if !self.require_enum_zero_value {
            return None;
        }
        if let Some((name, num)) = extract_enum_value(trimmed) {
            if num == 0 {
                if !name.ends_with("_UNSPECIFIED") && !name.ends_with("_UNKNOWN") {
                    return Some(
                        ScannerIssue::new(
                            "proto-enum-zero-value",
                            "warning",
                            rel,
                            format!(
                                "Enum zero value '{}' should end with _UNSPECIFIED or _UNKNOWN",
                                name
                            ),
                        )
                        .at_line(line),
                    );
                }
            }
        }
        None
    }
}

fn is_field_line(trimmed: &str) -> bool {
    if trimmed.starts_with("message ") || trimmed.starts_with("enum ") {
        return false;
    }
    if trimmed.starts_with("service ") || trimmed.starts_with("rpc ") {
        return false;
    }
    if trimmed.starts_with("option ") || trimmed.starts_with("reserved ") {
        return false;
    }
    if trimmed.starts_with("import ") || trimmed.starts_with("package ") {
        return false;
    }
    if trimmed.starts_with("oneof ") {
        return false;
    }
    if trimmed.contains('=') && trimmed.contains(';') {
        let type_part = trimmed.split_whitespace().next().unwrap_or("");
        let known_types = [
            "string", "int32", "int64", "uint32", "uint64", "bool", "bytes", "float", "double",
            "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64", "map",
        ];
        if known_types.contains(&type_part)
            || type_part.chars().next().map_or(false, |c| c.is_uppercase())
        {
            return true;
        }
    }
    false
}

fn extract_field_name(trimmed: &str) -> Option<String> {
    if !trimmed.contains('=') || !trimmed.contains(';') {
        return None;
    }
    let before_eq = trimmed.split('=').next()?;
    let parts: Vec<&str> = before_eq.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    Some(parts[parts.len() - 1].to_string())
}

fn extract_field_number(trimmed: &str) -> Option<(u32, bool)> {
    if !trimmed.contains('=') || !trimmed.contains(';') {
        return None;
    }
    let after_eq = trimmed.split('=').nth(1)?;
    let num_str = after_eq.split_whitespace().next()?.trim_end_matches(';');
    let num: u32 = num_str.parse().ok()?;
    let is_deprecated =
        trimmed.contains("deprecated = true") || trimmed.contains("deprecated=true");
    Some((num, is_deprecated))
}

fn extract_message_name(trimmed: &str) -> Option<String> {
    if let Some(rest) = trimmed.strip_prefix("message ") {
        let name = rest.split('{').next()?.trim();
        if name.is_empty() {
            return None;
        }
        return Some(name.to_string());
    }
    None
}

fn extract_enum_value(trimmed: &str) -> Option<(String, u32)> {
    if !trimmed.contains('=') || !trimmed.contains(';') {
        return None;
    }
    let before_eq = trimmed.split('=').next()?.trim();
    let name = before_eq.split_whitespace().last()?;
    let after_eq = trimmed.split('=').nth(1)?;
    let num_str = after_eq.split_whitespace().next()?.trim_end_matches(';');
    let num: u32 = num_str.parse().ok()?;
    Some((name.to_string(), num))
}

fn extract_import(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("import ")?;
    let rest = rest
        .trim_start_matches("public ")
        .trim_start_matches("weak ");
    let rest = rest.trim();
    let rest = rest.trim_end_matches(';').trim();
    let rest = rest.trim_matches('"');
    if rest.is_empty() {
        return None;
    }
    Some(rest.to_string())
}

fn extract_reserved_numbers(trimmed: &str) -> Option<Vec<u32>> {
    if !trimmed.starts_with("reserved ") {
        return None;
    }
    let rest = trimmed.strip_prefix("reserved ")?;
    let rest = rest.trim_end_matches(';').trim();
    let mut nums = Vec::new();
    for part in rest.split(',') {
        let part = part.trim();
        if part.contains("..") {
            let bounds: Vec<&str> = part.split("..").collect();
            if bounds.len() == 2 {
                if let (Ok(start), Ok(end)) = (
                    bounds[0].trim().parse::<u32>(),
                    bounds[1].trim().parse::<u32>(),
                ) {
                    for n in start..=end {
                        nums.push(n);
                    }
                }
            }
        } else if let Ok(n) = part.parse::<u32>() {
            nums.push(n);
        }
    }
    if nums.is_empty() {
        return None;
    }
    Some(nums)
}

fn find_unused_imports<'a>(imports: &[&'a str], content: &str) -> Vec<&'a str> {
    let mut unused = Vec::new();
    let body: String = content
        .lines()
        .filter(|l| !l.trim().starts_with("import "))
        .collect::<Vec<_>>()
        .join("\n");
    for imp in imports {
        let basename = imp.rsplit('/').next().unwrap_or(imp);
        let basename = basename.trim_end_matches(".proto");
        let search = basename.replace('-', "_");
        if !body.contains(&search) {
            unused.push(*imp);
        }
    }
    unused
}

fn is_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_numeric() || c == '_')
        && !s.starts_with('_')
}

fn is_pascal_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map_or(false, |c| c.is_uppercase())
        && s.chars().all(|c| c.is_alphanumeric())
}

impl Default for ProtobufLintScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clean_proto_file_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\npackage com.example.user;\n\nmessage User {\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_package() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\n\nmessage User {\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-package-present"));
        Ok(())
    }

    #[test]
    fn flags_proto2_syntax() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto2\";\npackage com.example;\n\nmessage User {\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-syntax-version"));
        Ok(())
    }

    #[test]
    fn flags_non_snake_case_field() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\npackage com.example;\n\nmessage User {\n  string userId = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-field-naming"));
        Ok(())
    }

    #[test]
    fn flags_non_pascal_case_message() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\npackage com.example;\n\nmessage user_message {\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-message-naming"));
        Ok(())
    }

    #[test]
    fn flags_enum_zero_value_not_unspecified() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("status.proto"),
            "syntax = \"proto3\";\npackage com.example;\n\nenum Status {\n  STATUS_ACTIVE = 0;\n  STATUS_INACTIVE = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-enum-zero-value"));
        Ok(())
    }

    #[test]
    fn accepts_enum_zero_value_with_unspecified() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("status.proto"),
            "syntax = \"proto3\";\npackage com.example;\n\nenum Status {\n  STATUS_UNSPECIFIED = 0;\n  STATUS_ACTIVE = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "proto-enum-zero-value"));
        Ok(())
    }

    #[test]
    fn flags_reserved_collision() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\npackage com.example;\n\nmessage User {\n  reserved 1;\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "proto-no-reserved-collision"));
        Ok(())
    }

    #[test]
    fn flags_deprecated_field_without_reserved() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\npackage com.example;\n\nmessage User {\n  string old_field = 1 [deprecated = true];\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "proto-no-deprecated-fields"));
        Ok(())
    }

    #[test]
    fn flags_unused_import() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto3\";\npackage com.example;\nimport \"google/protobuf/timestamp.proto\";\n\nmessage User {\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-imports-used"));
        Ok(())
    }

    #[test]
    fn silent_when_no_proto_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "no proto here")?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_proto_file_flags_missing_syntax_and_package() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("empty.proto"), "")?;
        let scanner = ProtobufLintScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "proto-syntax-version"));
        assert!(issues.iter().any(|i| i.rule == "proto-package-present"));
        Ok(())
    }

    #[test]
    fn config_can_disable_proto3_requirement() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("user.proto"),
            "syntax = \"proto2\";\npackage com.example;\n\nmessage User {\n  string user_id = 1;\n}\n",
        )?;
        let scanner = ProtobufLintScanner::with_config(false, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "proto-syntax-version"));
        Ok(())
    }
}
