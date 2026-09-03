use anyhow::Result as AnyhowResult;
use std::path::Path;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

/// Scan YAML `content` for hardcoded secret-like values assigned to keys whose
/// names contain `password`, `token`, `api_key`, or `secret`. Returns a list of
/// `(line_number, message)` tuples (1-based line numbers) for each hit.
///
/// This is a generic, line-oriented heuristic — it intentionally avoids pulling
/// in a full YAML parser so it can be reused by every YAML scanner regardless
/// of their typed `serde_yaml` structures. Values that reference environment
/// variables (`${...}`) or are empty are ignored.
pub fn detect_yaml_secrets(content: &str) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let key = trimmed[..colon].trim().trim_matches('"').trim_matches('\'');
        let lower = key.to_lowercase();
        let looks_secret = lower.contains("password")
            || lower.contains("token")
            || lower.contains("api_key")
            || lower.contains("apikey")
            || lower.contains("secret");
        if !looks_secret {
            continue;
        }
        let value = trimmed[colon + 1..].trim();
        if value.is_empty() || value == "|" || value == ">" {
            continue;
        }
        if value.contains("${") || value.contains("{{") || value.contains("<<") {
            continue;
        }
        let cleaned = value.trim_matches('"').trim_matches('\'');
        if cleaned.is_empty() {
            continue;
        }
        hits.push((
            i + 1,
            format!(
                "hardcoded secret-like value for '{}' detected; use an env reference",
                key
            ),
        ));
    }
    hits
}

pub type Result<T> = AnyhowResult<T>;

#[derive(Error, Debug)]
pub enum ProjectLintError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

pub fn get_project_root() -> Result<std::path::PathBuf> {
    let current_dir = std::env::current_dir()?;

    // Walk up the directory tree to find a git repository
    let mut path = current_dir.clone();
    while path.parent().is_some() {
        if path.join(".git").exists() {
            return Ok(path);
        }
        path = path.parent().unwrap().to_path_buf();
    }

    Err(anyhow::anyhow!(
        "No git repository found in current directory or parents"
    ))
}

pub fn get_config_dir() -> Result<std::path::PathBuf> {
    // First try project-specific config
    let project_root = get_project_root()?;
    let project_config = project_root.join(".config").join("project-lint");
    if project_config.exists() {
        return Ok(project_config);
    }

    // Fallback to XDG config home
    if let Some(config_dir) = dirs::config_dir() {
        let xdg_config = config_dir.join("project-lint");
        return Ok(xdg_config);
    }

    // Final fallback to ~/.config/project-lint
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home.join(".config").join("project-lint"))
}

pub fn matches_pattern(file_name: &str, pattern: &str) -> bool {
    if pattern.starts_with('*') && pattern.ends_with('*') {
        file_name.contains(&pattern[1..pattern.len() - 1])
    } else if pattern.starts_with('*') {
        file_name.ends_with(&pattern[1..])
    } else if pattern.ends_with('*') {
        file_name.starts_with(&pattern[..pattern.len() - 1])
    } else {
        file_name == pattern
    }
}

/// Returns true if `pattern` contains glob metacharacters (`*`, `?`, `[`).
pub fn is_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

/// Check whether any file matching `spec` exists relative to `project_root`.
///
/// `spec` may be a plain relative path (e.g. `"Cargo.toml"`) or a glob pattern
/// (e.g. `"next.config.*"`). For globs, only direct children of the project
/// root are considered (depth-1 match), which is the common case for config
/// files. Plain paths are checked with a direct filesystem stat.
pub fn path_exists_glob(project_root: &std::path::Path, spec: &str) -> bool {
    if !is_glob(spec) {
        return project_root.join(spec).exists();
    }

    // Glob: match direct children of project_root.
    if let Ok(entries) = std::fs::read_dir(project_root) {
        if let Ok(pattern) = glob::Pattern::new(spec) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if pattern.matches(&name_str) {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Centralized exclusion list for WalkDir-based scanners
// ---------------------------------------------------------------------------

/// Default directories excluded from all WalkDir-based scanners. These cover
/// build artifacts, dependency directories, and VCS internals that should
/// never be linted. `vendor` is intentionally absent — it is configurable via
/// `ExclusionConfig.allow_vendor` because some projects have first-party
/// `vendor/` directories.
pub const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".nuxt",
    ".svelte-kit",
    ".git",
    ".devbox/gen",
    ".cache",
    "coverage",
];

/// Assemble the full exclusion list from the defaults, user-provided extra
/// excludes, and the vendor toggle. When `allow_vendor` is false (the
/// default), `vendor` is appended so Go vendored dependencies are skipped.
/// When true, `vendor` is omitted so first-party `vendor/` dirs are scanned.
pub fn build_exclusions(extra_excludes: &[String], allow_vendor: bool) -> Vec<String> {
    let mut excluded: Vec<String> = DEFAULT_EXCLUDED_DIRS
        .iter()
        .map(|s| s.to_string())
        .collect();
    if !allow_vendor {
        excluded.push("vendor".to_string());
    }
    for ex in extra_excludes {
        let trimmed = ex.trim_matches('/');
        if !trimmed.is_empty() && !excluded.iter().any(|e| e == trimmed) {
            excluded.push(trimmed.to_string());
        }
    }
    excluded
}

/// Returns true if `rel` (a relative path string with `/` separators) is
/// under one of the excluded directories. This is the drop-in replacement for
/// the inline `rel_str.starts_with("target/") || rel_str.starts_with(".git/")`
/// pattern used by existing scanners.
///
/// Handles both single-segment exclusions (`target`, `node_modules`) and
/// multi-segment exclusions (`.devbox/gen`). A path equals an exclusion or
/// starts with `<exclusion>/`.
pub fn is_excluded_rel(rel: &str, excluded: &[String]) -> bool {
    let rel = rel.trim_start_matches("./");
    for ex in excluded {
        let ex = ex.trim_matches('/');
        if ex.is_empty() {
            continue;
        }
        if rel == ex || rel.starts_with(&format!("{}/", ex)) {
            return true;
        }
    }
    false
}

/// Returns true if the `DirEntry`'s path relative to `root` is under one of
/// the excluded directories. Used with `WalkDir::filter_entry` to prune
/// excluded directories during traversal (more efficient than post-hoc
/// filtering because children of excluded dirs are never visited).
pub fn is_excluded_entry(entry: &DirEntry, root: &Path, excluded: &[String]) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let rel = entry
        .path()
        .strip_prefix(root)
        .unwrap_or(entry.path())
        .to_string_lossy();
    is_excluded_rel(&rel, excluded)
}

/// Build a `WalkDir` iterator over `root` with excluded directories pruned via
/// `filter_entry`. Only directories are pruned; files inside non-excluded
/// directories are yielded. The iterator yields `DirEntry` values (errors are
/// filtered out via `filter_map`).
///
/// `max_depth` controls traversal depth (existing scanners use 3–6; 4 is the
/// common default). Pass `usize::MAX` for an unbounded walk.
pub fn walk_project<'a>(
    root: &'a Path,
    excluded: &'a [String],
    max_depth: usize,
) -> impl Iterator<Item = DirEntry> + 'a {
    WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(move |e| !is_excluded_entry(e, root, excluded))
        .filter_map(|e| e.ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn default_excluded_dirs_contains_build_artifacts() {
        // Assert every entry in DEFAULT_EXCLUDED_DIRS is present by name.
        for dir in DEFAULT_EXCLUDED_DIRS {
            assert!(
                DEFAULT_EXCLUDED_DIRS.contains(dir),
                "DEFAULT_EXCLUDED_DIRS should contain {:?}",
                dir
            );
        }
        // Explicitly assert each of the 12 known entries so a future
        // accidental removal is caught by name.
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&"node_modules"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&"target"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&"dist"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&"build"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".next"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".turbo"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".nuxt"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".svelte-kit"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".git"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".devbox/gen"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&".cache"));
        assert!(DEFAULT_EXCLUDED_DIRS.contains(&"coverage"));
    }

    #[test]
    fn default_excluded_dirs_omits_vendor() {
        // vendor is configurable, not in the default list
        assert!(!DEFAULT_EXCLUDED_DIRS.contains(&"vendor"));
    }

    #[test]
    fn build_exclusions_includes_vendor_by_default() {
        let excluded = build_exclusions(&[], false);
        assert!(excluded.iter().any(|e| e == "vendor"));
    }

    #[test]
    fn build_exclusions_omits_vendor_when_allowed() {
        let excluded = build_exclusions(&[], true);
        assert!(!excluded.iter().any(|e| e == "vendor"));
    }

    #[test]
    fn build_exclusions_appends_extra_excludes() {
        let excluded = build_exclusions(&["my-build-dir".into(), "generated".into()], false);
        assert!(excluded.iter().any(|e| e == "my-build-dir"));
        assert!(excluded.iter().any(|e| e == "generated"));
    }

    #[test]
    fn build_exclusions_deduplicates() {
        let excluded = build_exclusions(&["target".into(), "node_modules".into()], false);
        let target_count = excluded.iter().filter(|e| *e == "target").count();
        assert_eq!(target_count, 1);
    }

    #[test]
    fn build_exclusions_trims_and_ignores_empty() {
        let excluded = build_exclusions(&["  ".into(), "/foo/".into()], false);
        assert!(excluded.iter().any(|e| e == "foo"));
        assert!(!excluded.iter().any(|e| e.is_empty()));
    }

    #[test]
    fn is_excluded_rel_matches_single_segment() {
        let excluded = build_exclusions(&[], false);
        assert!(is_excluded_rel("target/foo.rs", &excluded));
        assert!(is_excluded_rel("node_modules/pkg/package.json", &excluded));
        assert!(is_excluded_rel("target", &excluded));
    }

    #[test]
    fn is_excluded_rel_matches_every_default_dir() {
        // Prove every entry in DEFAULT_EXCLUDED_DIRS is actually skipped by
        // the matching logic — both as a bare dir and with a child path.
        let excluded = build_exclusions(&[], false);
        for dir in DEFAULT_EXCLUDED_DIRS {
            let bare = *dir;
            let child = format!("{}/some-file", dir);
            assert!(
                is_excluded_rel(bare, &excluded),
                "is_excluded_rel should match bare {:?}",
                bare
            );
            assert!(
                is_excluded_rel(&child, &excluded),
                "is_excluded_rel should match child {:?}",
                child
            );
        }
        // Spot-check the six dirs that were previously unasserted by name.
        assert!(is_excluded_rel(".next/server/app.js", &excluded));
        assert!(is_excluded_rel(".turbo/cache.json", &excluded));
        assert!(is_excluded_rel(".nuxt/dist/index.js", &excluded));
        assert!(is_excluded_rel(".svelte-kit/output.json", &excluded));
        assert!(is_excluded_rel(".cache/foo", &excluded));
        assert!(is_excluded_rel("coverage/lcov.info", &excluded));
    }

    #[test]
    fn is_excluded_rel_matches_multi_segment() {
        let excluded = build_exclusions(&[], false);
        assert!(is_excluded_rel(".devbox/gen/foo.nix", &excluded));
        assert!(is_excluded_rel(".devbox/gen", &excluded));
        // .devbox itself is NOT excluded — only .devbox/gen
        assert!(!is_excluded_rel(".devbox/config.json", &excluded));
    }

    #[test]
    fn is_excluded_rel_does_not_match_non_excluded() {
        let excluded = build_exclusions(&[], false);
        assert!(!is_excluded_rel("src/main.rs", &excluded));
        assert!(!is_excluded_rel("Cargo.toml", &excluded));
        assert!(!is_excluded_rel("packages/foo/Cargo.toml", &excluded));
    }

    #[test]
    fn is_excluded_rel_respects_vendor_toggle() {
        let excluded = build_exclusions(&[], true);
        assert!(!is_excluded_rel("vendor/foo.go", &excluded));
        let excluded = build_exclusions(&[], false);
        assert!(is_excluded_rel("vendor/foo.go", &excluded));
    }

    #[test]
    fn is_excluded_rel_strips_dot_slash_prefix() {
        let excluded = build_exclusions(&[], false);
        assert!(is_excluded_rel("./target/foo.rs", &excluded));
    }

    #[test]
    fn walk_project_skips_excluded_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("Cargo.toml"), "").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("target/debug/app"), "").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/package.json"), "{}").unwrap();
        // Create the remaining default-excluded dirs with a sentinel file.
        for dir in [
            "dist",
            "build",
            ".next",
            ".turbo",
            ".nuxt",
            ".svelte-kit",
            ".cache",
            "coverage",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
            fs::write(root.join(dir).join("sentinel"), "").unwrap();
        }
        // .devbox/gen is multi-segment; .git is also multi-segment-capable.
        fs::create_dir_all(root.join(".devbox/gen")).unwrap();
        fs::write(root.join(".devbox/gen/foo.nix"), "").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"), "").unwrap();

        let excluded = build_exclusions(&[], false);
        let names: Vec<String> = walk_project(root, &excluded, 4)
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(root)
                    .unwrap_or(e.path())
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert!(names.iter().any(|p| p == "Cargo.toml"));
        assert!(names.iter().any(|p| p == "src/main.rs"));
        // Every default-excluded dir must be absent from the walked set.
        for dir in DEFAULT_EXCLUDED_DIRS {
            let needle = format!("{}/", dir);
            assert!(
                !names.iter().any(|p| p.starts_with(&needle) || p == *dir),
                "walk_project should skip {:?} but found a hit: {:?}",
                dir,
                names.iter().find(|p| p.starts_with(&needle) || *p == dir)
            );
        }
    }

    #[test]
    fn walk_project_respects_extra_excludes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("generated")).unwrap();
        fs::write(root.join("generated/out.rs"), "").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();

        let excluded = build_exclusions(&["generated".into()], false);
        let names: Vec<String> = walk_project(root, &excluded, 4)
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(root)
                    .unwrap_or(e.path())
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert!(names.iter().any(|p| p == "src/main.rs"));
        assert!(!names.iter().any(|p| p.contains("generated/")));
    }

    #[test]
    fn walk_project_respects_max_depth() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("a/b/c/d")).unwrap();
        fs::write(root.join("a/b/c/d/deep.txt"), "").unwrap();
        fs::write(root.join("a/shallow.txt"), "").unwrap();

        let excluded = build_exclusions(&[], false);
        let names: Vec<String> = walk_project(root, &excluded, 2)
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(root)
                    .unwrap_or(e.path())
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert!(names.iter().any(|p| p == "a/shallow.txt"));
        assert!(!names.iter().any(|p| p.contains("deep.txt")));
    }

    #[test]
    fn detect_yaml_secrets_flags_password_and_token() {
        let content = "name: app\ndb_password: hunter2\ntoken: abc123\n";
        let hits = detect_yaml_secrets(content);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0, 2);
        assert_eq!(hits[1].0, 3);
    }

    #[test]
    fn detect_yaml_secrets_ignores_env_refs() {
        let content = "db_password: ${DB_PASSWORD}\ntoken: {{ TOKEN }}\nsecret: <<\n";
        let hits = detect_yaml_secrets(content);
        assert!(hits.is_empty(), "env refs should be ignored: {:?}", hits);
    }

    #[test]
    fn detect_yaml_secrets_ignores_empty_and_comments() {
        let content = "# password: not-a-secret\nempty_password:\napi_key: |\n";
        let hits = detect_yaml_secrets(content);
        assert!(hits.is_empty(), "empty/comment lines ignored: {:?}", hits);
    }

    #[test]
    fn detect_yaml_secrets_ignores_list_items() {
        let content = "- password\n- token: value\n";
        let hits = detect_yaml_secrets(content);
        assert!(hits.is_empty(), "list items ignored: {:?}", hits);
    }
}
