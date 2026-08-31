//! Skill markdown scanner — validates `SKILL.md` files in the skills-src
//! ecosystem. Enforces three rules:
//!
//! 1. **Body line count** — the wrapper body (everything after the YAML
//!    frontmatter) must not exceed a configurable line limit (default 80).
//!    The wrapper pattern keeps `SKILL.md` thin: frontmatter + a request to
//!    run `scripts/refresh.sh`, which prints the real body (`INSTRUCTIONS.md`).
//!    Frontmatter is metadata (tags, `see-also` lists) and is not counted.
//! 2. **refresh.sh presence** — every `SKILL.md` must have a sibling
//!    `scripts/refresh.sh` that materializes the skill body.
//! 3. **Frontmatter validity** — the YAML frontmatter must start/end with
//!    `---` delimiters and contain the required fields `name`, `description`,
//!    and `version`, matching the convention used by `ai-upsert` and the rest
//!    of the upsert skill family.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use std::path::Path;
use walkdir::WalkDir;

/// Default maximum body line count for a wrapper-pattern `SKILL.md`.
pub const DEFAULT_MAX_BODY_LINES: usize = 80;

pub struct SkillMarkdownScanner {
    max_body_lines: usize,
    require_refresh_script: bool,
    /// Directory names that are fully exempt from scanning (e.g. `references/`).
    /// When empty, only `target/` and `.git/` are skipped.
    exempt_dirs: Vec<String>,
}

impl SkillMarkdownScanner {
    pub fn new() -> Self {
        Self {
            max_body_lines: DEFAULT_MAX_BODY_LINES,
            require_refresh_script: true,
            exempt_dirs: Vec::new(),
        }
    }

    pub fn with_config(
        max_body_lines: usize,
        require_refresh_script: bool,
        exempt_dirs: Vec<String>,
    ) -> Self {
        Self {
            max_body_lines,
            require_refresh_script,
            exempt_dirs,
        }
    }

    /// Scan a project root for `SKILL.md` files and validate each one.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in WalkDir::new(root)
            .max_depth(6)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.to_string_lossy();
            if rel_str.starts_with("target/") || rel_str.starts_with(".git/") {
                continue;
            }
            if self.is_exempt(&rel_str) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "SKILL.md" {
                continue;
            }
            issues.extend(self.scan_skill_file(path, &rel_str));
        }

        Ok(issues)
    }

    fn is_exempt(&self, rel: &str) -> bool {
        for dir in &self.exempt_dirs {
            let trimmed = dir.trim_matches('/');
            if trimmed.is_empty() {
                continue;
            }
            // Match a path segment equal to the exempt dir name.
            if rel.split('/').any(|seg| seg == trimmed) {
                return true;
            }
        }
        false
    }

    fn scan_skill_file(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let Ok(content) = std::fs::read_to_string(path) else {
            return issues;
        };

        // Rule 3: frontmatter validity (also locates the body for rule 1).
        let body = match extract_body_after_frontmatter(&content) {
            Ok(body) => body,
            Err(err) => {
                issues.push(ScannerIssue::new("skill-frontmatter", "error", rel, err));
                return issues;
            }
        };

        // Validate required frontmatter fields.
        if let Err(field_errs) = validate_required_frontmatter_fields(&content) {
            for e in field_errs {
                issues.push(ScannerIssue::new("skill-frontmatter", "error", rel, e));
            }
        }

        // Rule 1: body line count.
        let body_lines = body.lines().count();
        if body_lines > self.max_body_lines {
            issues.push(
                ScannerIssue::new(
                    "skill-body-too-long",
                    "warning",
                    rel,
                    format!(
                        "SKILL.md body is {} lines (limit {}); move detail into INSTRUCTIONS.md via refresh.sh",
                        body_lines, self.max_body_lines,
                    ),
                )
                .at_line(1),
            );
        }

        // Rule 2: refresh.sh presence.
        if self.require_refresh_script {
            let skill_dir = path.parent().unwrap_or_else(|| Path::new(""));
            let refresh = skill_dir.join("scripts").join("refresh.sh");
            if !refresh.exists() {
                issues.push(ScannerIssue::new(
                    "skill-missing-refresh",
                    "error",
                    rel,
                    "missing sibling scripts/refresh.sh (wrapper pattern requires it to print INSTRUCTIONS.md)",
                ));
            }
        }

        issues
    }
}

impl Default for SkillMarkdownScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the content after the YAML frontmatter block.
/// Returns `Err(message)` if the frontmatter is malformed.
///
/// The opening `---` must be at the very start of the file. The closing `---`
/// must be the sole content of its line (trailing whitespace tolerated). Both
/// `\n` and `\r\n` line endings are handled.
fn extract_body_after_frontmatter(content: &str) -> std::result::Result<&str, String> {
    if !content.starts_with("---") {
        return Err("missing frontmatter block (must start with ---)".to_string());
    }
    // Walk lines preserving their terminators so byte offsets stay exact for
    // both `\n` and `\r\n`. The first line is the opening `---`; we scan the
    // remainder for the closing delimiter.
    let mut offset = 0usize; // byte position within `content` of the current line start
    let mut first = true;
    for line in content.split_inclusive('\n') {
        if first {
            first = false;
            offset += line.len();
            continue;
        }
        // `line` includes its trailing `\n` (and any `\r` before it). Strip the
        // terminator(s) to inspect the line's content.
        let stripped = line.strip_suffix('\n').unwrap_or(line);
        let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);
        if stripped.trim_end() == "---" {
            // Body starts after this line.
            let body_start = offset + line.len();
            return Ok(&content[body_start..]);
        }
        offset += line.len();
    }
    Err("incomplete frontmatter block (missing closing ---)".to_string())
}

/// Validate that the frontmatter contains the required fields `name`,
/// `description`, and `version`. Uses simple top-level key detection (a line
/// starting with `key:` at indentation depth 0), consistent with the existing
/// `markdown_frontmatter` scanner's line-based parser.
fn validate_required_frontmatter_fields(content: &str) -> std::result::Result<(), Vec<String>> {
    if !content.starts_with("---") {
        return Err(vec![
            "missing frontmatter block (must start with ---)".to_string()
        ]);
    }
    // Collect the frontmatter lines (between the opening and closing `---`).
    let mut frontmatter_lines: Vec<&str> = Vec::new();
    let mut first = true;
    let mut found_closer = false;
    for line in content.split_inclusive('\n') {
        let stripped = line.strip_suffix('\n').unwrap_or(line);
        let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);
        if first {
            first = false;
            continue;
        }
        if stripped.trim_end() == "---" {
            found_closer = true;
            break;
        }
        frontmatter_lines.push(stripped);
    }
    if !found_closer {
        return Err(vec![
            "incomplete frontmatter block (missing closing ---)".to_string()
        ]);
    }

    let mut has_name = false;
    let mut has_description = false;
    let mut has_version = false;
    let mut errs = Vec::new();

    for line in &frontmatter_lines {
        // Only top-level keys: no leading whitespace before the colon key.
        if line.is_empty() || line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim();
            let value = line[colon + 1..].trim();
            match key {
                "name" => {
                    has_name = true;
                    if value.is_empty() {
                        errs.push("frontmatter field 'name' is empty".to_string());
                    }
                }
                "description" => {
                    has_description = true;
                    if value.is_empty() {
                        errs.push("frontmatter field 'description' is empty".to_string());
                    }
                }
                "version" => {
                    has_version = true;
                    if value.is_empty() {
                        errs.push("frontmatter field 'version' is empty".to_string());
                    }
                }
                _ => {}
            }
        }
    }

    if !has_name {
        errs.push("missing required frontmatter field: name".to_string());
    }
    if !has_description {
        errs.push("missing required frontmatter field: description".to_string());
    }
    if !has_version {
        errs.push("missing required frontmatter field: version".to_string());
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_skill(dir: &Path, body: &str, with_refresh: bool) {
        let skill_dir = dir.join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), body).unwrap();
        if with_refresh {
            let scripts = skill_dir.join("scripts");
            std::fs::create_dir_all(&scripts).unwrap();
            std::fs::write(
                scripts.join("refresh.sh"),
                "#!/usr/bin/env bash\ncat INSTRUCTIONS.md\n",
            )
            .unwrap();
        }
    }

    fn valid_frontmatter() -> &'static str {
        "---\nname: my-skill\ndescription: A test skill\nversion: 1.0.0\ntags:\n  - x\n---\n"
    }

    #[test]
    fn valid_short_skill_passes() -> Result<()> {
        let dir = TempDir::new()?;
        let body = format!("{}# Body\n\nRun refresh.sh.\n", valid_frontmatter());
        write_skill(&dir.path(), &body, true);
        let scanner = SkillMarkdownScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_missing_refresh_script() -> Result<()> {
        let dir = TempDir::new()?;
        let body = format!("{}# Body\n", valid_frontmatter());
        write_skill(&dir.path(), &body, false);
        let scanner = SkillMarkdownScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "skill-missing-refresh"));
        Ok(())
    }

    #[test]
    fn require_refresh_can_be_disabled() -> Result<()> {
        let dir = TempDir::new()?;
        let body = format!("{}# Body\n", valid_frontmatter());
        write_skill(&dir.path(), &body, false);
        let scanner = SkillMarkdownScanner::with_config(80, false, Vec::new());
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(!issues.iter().any(|i| i.rule == "skill-missing-refresh"));
        Ok(())
    }

    #[test]
    fn flags_body_over_limit() -> Result<()> {
        let dir = TempDir::new()?;
        // Body of 3 lines but limit 2.
        let body = format!("{}# Body\n\nline2\nline3\n", valid_frontmatter());
        write_skill(&dir.path(), &body, true);
        let scanner = SkillMarkdownScanner::with_config(2, true, Vec::new());
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "skill-body-too-long"));
        Ok(())
    }

    #[test]
    fn frontmatter_not_counted_toward_body() -> Result<()> {
        let dir = TempDir::new()?;
        // Large frontmatter (many tags) but tiny body — must not trigger.
        let mut fm =
            String::from("---\nname: my-skill\ndescription: A test skill\nversion: 1.0.0\ntags:\n");
        for i in 0..200 {
            fm.push_str(&format!("  - \"tag-{}\"\n", i));
        }
        fm.push_str("---\n# Body\n\nRun refresh.sh.\n");
        write_skill(&dir.path(), &fm, true);
        let scanner = SkillMarkdownScanner::new();
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_missing_frontmatter() -> Result<()> {
        let dir = TempDir::new()?;
        write_skill(&dir.path(), "# No frontmatter here\n", true);
        let scanner = SkillMarkdownScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "skill-frontmatter"));
        Ok(())
    }

    #[test]
    fn flags_missing_required_fields() -> Result<()> {
        let dir = TempDir::new()?;
        let body = "---\nname: my-skill\ndescription: A test skill\n---\n# Body\n";
        write_skill(&dir.path(), body, true);
        let scanner = SkillMarkdownScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "skill-frontmatter" && i.message.contains("version")));
        Ok(())
    }

    #[test]
    fn flags_incomplete_frontmatter() -> Result<()> {
        let dir = TempDir::new()?;
        let body = "---\nname: my-skill\ndescription: A test skill\nversion: 1.0.0\n# no closing delimiter\n";
        write_skill(&dir.path(), body, true);
        let scanner = SkillMarkdownScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "skill-frontmatter" && i.message.contains("closing")));
        Ok(())
    }

    #[test]
    fn exempt_dirs_skip_skill_md() -> Result<()> {
        let dir = TempDir::new()?;
        // A SKILL.md inside references/ that would normally fail (no refresh).
        let skill_dir = dir.path().join("references").join("included").join("foo");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: foo\ndescription: x\nversion: 1.0.0\n---\n# Body\n",
        )
        .unwrap();
        let scanner = SkillMarkdownScanner::with_config(80, true, vec!["references".to_string()]);
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        Ok(())
    }

    #[test]
    fn extract_body_handles_crlf_and_trailing_whitespace() {
        let content = "---\r\nname: x\r\ndescription: y\r\nversion: 1.0.0\r\n--- \r\n# Body\r\n";
        let body = extract_body_after_frontmatter(content).unwrap();
        assert!(body.starts_with("# Body"));
    }

    #[test]
    fn extract_body_no_frontmatter_errors() {
        let err = extract_body_after_frontmatter("# just a body").unwrap_err();
        assert!(err.contains("missing frontmatter"));
    }

    #[test]
    fn default_max_body_lines_is_80() {
        assert_eq!(DEFAULT_MAX_BODY_LINES, 80);
        assert_eq!(SkillMarkdownScanner::new().max_body_lines, 80);
    }
}
