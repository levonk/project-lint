//! Tilde-in-PATH scanner — detects `~` used in `PATH` (and `*PATH` /
//! `*PATHS`-style search-path variables) where the tilde will never be
//! expanded into the user's home directory.
//!
//! Tilde expansion only happens for *unquoted* tildes, and in assignments
//! only immediately after `=` or `:`. A quoted tilde in
//! `export PATH="$PATH:~/.local/bin"` stays literal, so the shell ends up
//! searching `./~/.local/bin` relative to whatever the current working
//! directory happens to be — broken, and a security risk when a writable
//! `~` directory can shadow executables. The same applies to file formats
//! with no tilde expansion at all (`Dockerfile` `ENV`, `.env` files, YAML
//! `PATH:` keys, justfile/make exports).
//!
//! Reference: <https://disconnect3d.pl//2026/10/02/dont-put-tilde-in-your-path/>
//!
//! Rules:
//! - `no-quoted-tilde-in-path` (error) — `~` inside quotes (or
//!   backslash-escaped) in a PATH assignment never expands.
//! - `no-literal-tilde-in-path` (error) — `~` in a PATH value where the file
//!   format performs no tilde expansion (Dockerfile `ENV`, `.env`, YAML
//!   `PATH:` keys, justfile/make top-level exports).
//! - `fragile-tilde-in-path` (info) — unquoted `~` in a shell assignment;
//!   expands via bash/zsh `:`/`=` rules but breaks on whitespace and in
//!   helper calls like `PATH_add`.
//!
//! The scanner covers shell scripts (`*.sh`, `*.bash`, `*.zsh`, `*.fish`,
//! `*.ksh`), rc/profile files (`.bashrc`, `.zshrc`, `.profile`, `.envrc`, …),
//! template variants of any of these (`*.tmpl`, `*.template`, `*.j2`,
//! chezmoi `dot_*` / `run_*` / `private_*` names), Nix files, YAML files,
//! `Dockerfile`/`Containerfile`, `.env` files, justfiles, and Makefiles.
//! Extensionless files are also scanned when their shebang names a shell.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;
use tracing::debug;

/// Template suffixes stripped before classification so `setup.sh.tmpl` or
/// `dot_bashrc.j2` are still recognized as shell content.
const TEMPLATE_SUFFIXES: &[&str] = &[
    ".tmpl",
    ".template",
    ".tpl",
    ".j2",
    ".jinja",
    ".jinja2",
    ".gotmpl",
    ".mustache",
];

/// chezmoi-style attribute prefixes stripped before classification
/// (`private_dot_zshrc.tmpl` → `.zshrc`). `dot_` is handled separately.
const CHEZMOI_PREFIXES: &[&str] = &[
    "private_",
    "readonly_",
    "empty_",
    "executable_",
    "encrypted_",
    "exact_",
    "literal_",
    "external_",
    "symlink_",
    "create_",
    "modify_",
    "script_",
    "template_",
    "run_once_",
    "run_after_",
    "run_before_",
    "run_",
    "once_",
    "onchange_",
    "before_",
    "after_",
];

/// File names (after template/chezmoi normalization, lowercase) whose
/// contents are evaluated by a POSIX-ish shell.
const SHELL_NAMES: &[&str] = &[
    ".envrc",
    "envrc",
    ".bashrc",
    "bashrc",
    "bash.bashrc",
    ".zshrc",
    "zshrc",
    ".bash_profile",
    ".bash_login",
    ".bash_logout",
    ".profile",
    "profile",
    ".xprofile",
    ".xsessionrc",
    ".zprofile",
    ".zshenv",
    ".zlogin",
    ".zlogout",
    ".bash_aliases",
    ".aliases",
    ".bash_functions",
    "config.fish",
];

/// Extensions evaluated by a shell (or embedding shell snippets, like Nix
/// `shellHook` strings and workflow `run:` blocks).
const SHELL_EXTENSIONS: &[&str] = &["sh", "bash", "zsh", "fish", "ksh", "nix", "yml", "yaml"];

/// Extensions whose PATH assignments are stored literally (no tilde
/// expansion): dotenv files, make fragments, just modules.
const LITERAL_EXTENSIONS: &[&str] = &["env", "mk", "just"];

/// Helper functions/builtins that take a path argument instead of an
/// assignment — a quoted `~` never expands and even an unquoted one is
/// fragile (`PATH_add "~/bin"`, `pathmunge ~/bin`).
const PATH_HELPERS: &[&str] = &[
    "PATH_add",
    "path_add",
    "pathmunge",
    "fish_add_path",
    "add_to_path",
    "prepend_to_path",
    "append_to_path",
    "path_prepend",
    "path_append",
];

/// How a `~` inside a PATH value is interpreted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Semantics {
    /// Shell-evaluated content: quoted/escaped `~` is literal, unquoted `~`
    /// expands only in assignment position (fragile).
    Shell,
    /// Stored literally: no tilde expansion at all (Dockerfile `ENV`,
    /// dotenv, YAML keys, justfile/make exports).
    Literal,
}

pub struct TildePathScanner {
    check_quoted: bool,
    check_literal: bool,
    check_fragile: bool,
    excluded: Vec<String>,
}

impl TildePathScanner {
    pub fn new() -> Self {
        Self {
            check_quoted: true,
            check_literal: true,
            check_fragile: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(check_quoted: bool, check_literal: bool, check_fragile: bool) -> Self {
        Self {
            check_quoted,
            check_literal,
            check_fragile,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        check_quoted: bool,
        check_literal: bool,
        check_fragile: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            check_quoted,
            check_literal,
            check_fragile,
            excluded,
        }
    }

    /// Scan a project root for files that may contain PATH assignments and
    /// lint each for non-expanding tildes.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 6).filter(|e| e.file_type().is_file()) {
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
            let file_sem = classify_name(&name);
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let file_sem = match file_sem {
                Some(s) => s,
                None if has_shell_shebang(&content) => Semantics::Shell,
                None => continue,
            };
            debug!("tilde_path scanning {}", rel);
            issues.extend(self.scan_file(&content, &rel, file_sem));
        }

        Ok(issues)
    }

    fn scan_file(&self, content: &str, rel: &str, file_sem: Semantics) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();
        let is_makefile = matches!(file_sem, Semantics::Literal) && is_makefile_name(rel);

        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Makefile recipe lines (tab-indented) are evaluated by the
            // shell even though top-level exports are literal.
            let sem = if is_makefile && line.starts_with('\t') {
                Semantics::Shell
            } else {
                file_sem
            };
            for hit in find_path_tildes(line) {
                let (rule, severity, message) = if hit.protected {
                    (
                        "no-quoted-tilde-in-path",
                        "error",
                        "quoted '~' in PATH never expands — the entry resolves to a literal './~' under the current directory; use $HOME instead",
                    )
                } else if sem == Semantics::Literal || hit.colon_key {
                    (
                        "no-literal-tilde-in-path",
                        "error",
                        "'~' in PATH is stored literally here (no tilde expansion in this file format); use an absolute path or $HOME",
                    )
                } else {
                    (
                        "fragile-tilde-in-path",
                        "info",
                        "unquoted '~' in PATH only expands in assignment position (breaks on whitespace and helper calls); use $HOME",
                    )
                };
                let enabled = match rule {
                    "no-quoted-tilde-in-path" => self.check_quoted,
                    "no-literal-tilde-in-path" => self.check_literal,
                    _ => self.check_fragile,
                };
                if enabled
                    && !issues
                        .iter()
                        .any(|si: &ScannerIssue| si.rule == rule && si.line == i + 1)
                {
                    issues.push(ScannerIssue::new(rule, severity, rel, message).at_line(i + 1));
                }
            }
        }

        issues
    }
}

impl Default for TildePathScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// A `~` found inside a PATH-assignment value.
struct TildeHit {
    /// Inside single/double quotes or backslash-escaped — never expands.
    protected: bool,
    /// Matched via `PATH:` key form (YAML/env-style) rather than `PATH=` —
    /// the value is stored literally regardless of quoting.
    colon_key: bool,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Find all `~` characters that fall inside PATH-assignment values on this
/// line. Handles `PATH=`, `PATH:=`, `PATH+=`, `*PATH=`, YAML-style `PATH:`,
/// fish `set PATH x`, Dockerfile `ENV PATH x`, and helper calls like
/// `PATH_add` / `pathmunge`.
fn find_path_tildes(line: &str) -> Vec<TildeHit> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut protected = vec![false; line.len() + 1];
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for &(i, c) in &chars {
        protected[i] = in_single || in_double || escaped;
        if escaped {
            escaped = false;
        } else if in_single {
            if c == '\'' {
                in_single = false;
            }
        } else if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                escaped = true;
            }
        } else if c == '\\' {
            escaped = true;
        } else if c == '\'' {
            in_single = true;
        } else if c == '"' {
            in_double = true;
        }
    }

    let mut hits = Vec::new();
    let mut idx = 0;
    while idx < chars.len() {
        let (s, c) = chars[idx];
        // Identifier tokens start at a non-identifier boundary.
        let prev_ok = s == 0 || !is_ident(line.as_bytes()[s - 1] as char);
        if !(c.is_ascii_alphabetic() || c == '_') || !prev_ok {
            idx += 1;
            continue;
        }
        let mut e = s + c.len_utf8();
        while e < line.len() && is_ident(line.as_bytes()[e] as char) {
            e += 1;
        }
        let tok = &line[s..e];

        let mut j = e;
        while j < line.len() && (line.as_bytes()[j] == b' ' || line.as_bytes()[j] == b'\t') {
            j += 1;
        }

        let (value_start, colon_key) = if PATH_HELPERS.contains(&tok) && j > e {
            (j, false)
        } else if tok.ends_with("PATH") || tok.ends_with("PATHS") {
            if j < line.len() && line.as_bytes()[j] == b'=' {
                // PATH= / MYPATH=
                (j + 1, false)
            } else if j + 1 < line.len()
                && (line.as_bytes()[j] == b':' || line.as_bytes()[j] == b'+')
                && line.as_bytes()[j + 1] == b'='
            {
                // PATH:= / PATH+=
                (j + 2, false)
            } else if j < line.len()
                && line.as_bytes()[j] == b':'
                && (j + 1 >= line.len()
                    || matches!(line.as_bytes()[j + 1], b' ' | b'\t' | b'"' | b'\''))
            {
                // YAML/env key form: `PATH: value`
                (j + 1, true)
            } else if tok == "PATH"
                && j > e
                && matches!(
                    line[..s].split_whitespace().next(),
                    Some("ENV" | "env" | "set")
                )
            {
                // `ENV PATH value` / `env PATH value` / `set PATH value` (fish)
                (j, false)
            } else {
                idx += 1;
                continue;
            }
        } else {
            idx += 1;
            continue;
        };

        // Value ends at an unquoted `;`, `&`, `|`, or ` #` comment.
        let mut vend = line.len();
        for &(k, c) in &chars {
            if k <= value_start {
                continue;
            }
            if protected[k] {
                continue;
            }
            if matches!(c, ';' | '&' | '|') || (c == '#' && line.as_bytes()[k - 1] == b' ') {
                vend = k;
                break;
            }
        }

        for &(k, c) in &chars {
            if k >= value_start && k < vend && c == '~' {
                hits.push(TildeHit {
                    protected: protected[k],
                    colon_key,
                });
            }
        }
        idx += 1;
    }

    hits
}

/// Normalize a file name (strip template suffixes and chezmoi prefixes,
/// map `dot_foo` → `.foo`) and classify its PATH semantics.
fn classify_name(name: &str) -> Option<Semantics> {
    let mut n = name.to_string();
    loop {
        let stripped = TEMPLATE_SUFFIXES
            .iter()
            .find_map(|s| n.strip_suffix(s).map(|r| r.to_string()));
        match stripped {
            Some(r) => n = r,
            None => break,
        }
    }
    loop {
        let stripped = CHEZMOI_PREFIXES
            .iter()
            .find_map(|p| n.strip_prefix(p).map(|r| r.to_string()));
        match stripped {
            Some(r) => n = r,
            None => break,
        }
    }
    if let Some(rest) = n.strip_prefix("dot_") {
        n = format!(".{}", rest);
    }
    let lower = n.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");

    if SHELL_NAMES.contains(&lower.as_str()) || lower.starts_with(".envrc.") {
        return Some(Semantics::Shell);
    }
    if lower.starts_with("dockerfile")
        || lower.starts_with("containerfile")
        || matches!(
            lower.as_str(),
            "justfile" | "makefile" | "gnumakefile" | ".env"
        )
        || lower.starts_with(".env.")
    {
        return Some(Semantics::Literal);
    }
    if LITERAL_EXTENSIONS.contains(&ext) {
        return Some(Semantics::Literal);
    }
    if SHELL_EXTENSIONS.contains(&ext) {
        return Some(Semantics::Shell);
    }
    None
}

fn is_makefile_name(rel: &str) -> bool {
    let name = Path::new(rel)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    matches!(name.as_str(), "makefile" | "gnumakefile") || name.ends_with(".mk")
}

/// Extensionless-file fallback: a `#!` first line whose interpreter is a
/// shell (including `#!/usr/bin/env <shell>` and `env -S` forms).
fn has_shell_shebang(content: &str) -> bool {
    let Some(first) = content.lines().next() else {
        return false;
    };
    let Some(rest) = first.strip_prefix("#!") else {
        return false;
    };
    let mut it = rest.split_whitespace();
    let mut cmd = it.next().unwrap_or("");
    if cmd == "env" || cmd.ends_with("/env") {
        cmd = "";
        for tok in it {
            if tok.starts_with('-') {
                continue;
            }
            cmd = tok;
            break;
        }
    }
    let base = cmd.rsplit('/').next().unwrap_or(cmd);
    matches!(
        base,
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "dash" | "ash"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    fn scan(dir: &TempDir) -> Result<Vec<ScannerIssue>> {
        TildePathScanner::new().scan(&dir.path().to_string_lossy())
    }

    #[test]
    fn clean_path_assignment_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nexport PATH=\"$HOME/bin:$PATH\"\nexport PATH=\"$PATH:$HOME/.local/bin\"\n",
        );
        assert!(scan(&dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn flags_quoted_tilde_in_path() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\nexport PATH=\"$PATH:~/.local/bin\"\n",
        );
        let issues = scan(&dir)?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "no-quoted-tilde-in-path" && i.severity == "error" && i.line == 2));
        Ok(())
    }

    #[test]
    fn flags_quoted_tilde_at_value_start() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), "run.sh", "export PATH=\"~/bin:$PATH\"\n");
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-quoted-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn flags_single_quoted_tilde() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), ".zshrc", "export PATH='~/bin:$PATH'\n");
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-quoted-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn flags_escaped_tilde() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), "run.sh", "export PATH=~/bin:$PATH\\:~/.x\n");
        let issues = scan(&dir)?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "no-quoted-tilde-in-path" || i.rule == "fragile-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn unquoted_tilde_is_info() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), "run.sh", "export PATH=$PATH:~/bin\n");
        let issues = scan(&dir)?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "fragile-tilde-in-path" && i.severity == "info"));
        assert!(!issues.iter().any(|i| i.severity == "error"));
        Ok(())
    }

    #[test]
    fn flags_path_add_quoted_tilde() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), ".envrc", "PATH_add \"~/.local/bin\"\n");
        let issues = scan(&dir)?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "no-quoted-tilde-in-path" && i.file == ".envrc"));
        Ok(())
    }

    #[test]
    fn template_files_scanned() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "install.sh.tmpl",
            "export PATH=\"~/bin:$PATH\"\n",
        );
        write(
            dir.path(),
            "dot_zshrc.tmpl",
            "export PATH=\"$PATH:~/.local/bin\"\n",
        );
        write(dir.path(), "private_dot_bashrc", "export PATH='~/x'\n");
        write(
            dir.path(),
            "run_once_setup.sh.tmpl",
            "export PATH=\"~/y:$PATH\"\n",
        );
        let issues = scan(&dir)?;
        let flagged: Vec<&str> = issues
            .iter()
            .filter(|i| i.rule == "no-quoted-tilde-in-path")
            .map(|i| i.file.as_str())
            .collect();
        assert!(flagged.contains(&"install.sh.tmpl"));
        assert!(flagged.contains(&"dot_zshrc.tmpl"));
        assert!(flagged.contains(&"private_dot_bashrc"));
        assert!(flagged.contains(&"run_once_setup.sh.tmpl"));
        Ok(())
    }

    #[test]
    fn dockerfile_env_tilde_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "Dockerfile",
            "FROM alpine\nENV PATH=/usr/local/bin:~/.local/bin\n",
        );
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-literal-tilde-in-path"
            && i.severity == "error"
            && i.file == "Dockerfile"));
        Ok(())
    }

    #[test]
    fn yaml_path_key_tilde_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "docker-compose.yml",
            "services:\n  app:\n    environment:\n      PATH: \"~/bin:/usr/bin\"\n",
        );
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-quoted-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn yaml_path_key_unquoted_tilde_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "docker-compose.yml",
            "services:\n  app:\n    environment:\n      PATH: ~/bin:/usr/bin\n",
        );
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-literal-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn env_file_tilde_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), ".env", "PATH=~/bin:/usr/bin\n");
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-literal-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn makefile_recipe_line_is_shell() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "Makefile",
            "export PATH := ~/bin\n\nrun:\n\texport PATH=$PATH:~/bin && foo\n",
        );
        let issues = scan(&dir)?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "no-literal-tilde-in-path" && i.line == 1));
        assert!(issues
            .iter()
            .any(|i| i.rule == "fragile-tilde-in-path" && i.line == 4));
        Ok(())
    }

    #[test]
    fn nix_shell_hook_quoted_tilde() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "shell.nix",
            "{ pkgs ? import <nixpkgs> {} }:\npkgs.mkShell { shellHook = '' export PATH=\"~/bin:$PATH\" ''; }\n",
        );
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-quoted-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn shebang_only_extensionless_script_scanned() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "bootstrap",
            "#!/usr/bin/env bash\nexport PATH=\"$PATH:~/bin\"\n",
        );
        let issues = scan(&dir)?;
        assert!(issues
            .iter()
            .any(|i| i.rule == "no-quoted-tilde-in-path" && i.file == "bootstrap"));
        Ok(())
    }

    #[test]
    fn non_shell_shebang_not_scanned() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), "tool", "#!/usr/bin/env python3\nPATH=\"~/x\"\n");
        assert!(scan(&dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn manpath_and_other_path_vars_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            ".bashrc",
            "export MANPATH=\"~/.local/share/man:$MANPATH\"\nexport PYTHONPATH=\"$PYTHONPATH:~/lib\"\n",
        );
        let issues = scan(&dir)?;
        assert_eq!(
            issues
                .iter()
                .filter(|i| i.rule == "no-quoted-tilde-in-path")
                .count(),
            2
        );
        Ok(())
    }

    #[test]
    fn non_path_vars_not_flagged() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "run.sh",
            "echo \"~/.config\"\nGREETING=\"hi ~\"\ncd ~\n",
        );
        assert!(scan(&dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn comment_lines_skipped() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "run.sh",
            "#!/usr/bin/env bash\n# export PATH=\"~/bin:$PATH\"\nexport PATH=\"$HOME/bin:$PATH\"\n",
        );
        assert!(scan(&dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn non_shell_files_skipped() -> Result<()> {
        let dir = TempDir::new()?;
        write(dir.path(), "notes.md", "export PATH=\"$PATH:~/bin\"\n");
        assert!(scan(&dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn yaml_run_block_quoted_tilde() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "ci.yml",
            "steps:\n  - run: export PATH=\"~/bin:$PATH\"\n",
        );
        let issues = scan(&dir)?;
        assert!(issues.iter().any(|i| i.rule == "no-quoted-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn config_disables_rules() -> Result<()> {
        let dir = TempDir::new()?;
        write(
            dir.path(),
            "run.sh",
            "export PATH=\"~/bin\"\nexport PATH=$PATH:~/x\n",
        );
        let scanner = TildePathScanner::with_config(false, false, false);
        assert!(scanner.scan(&dir.path().to_string_lossy())?.is_empty());
        let scanner = TildePathScanner::with_config(true, true, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().all(|i| i.rule != "fragile-tilde-in-path"));
        assert!(issues.iter().any(|i| i.rule == "no-quoted-tilde-in-path"));
        Ok(())
    }

    #[test]
    fn skips_excluded_dirs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::create_dir_all(dir.path().join("node_modules"))?;
        write(
            &dir.path().join("node_modules"),
            "x.sh",
            "export PATH=\"~/bin\"\n",
        );
        assert!(scan(&dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn classify_name_covers_templates() {
        assert_eq!(classify_name("setup.sh"), Some(Semantics::Shell));
        assert_eq!(classify_name("setup.sh.tmpl"), Some(Semantics::Shell));
        assert_eq!(classify_name("dot_bashrc.tmpl"), Some(Semantics::Shell));
        assert_eq!(classify_name("private_dot_zshrc"), Some(Semantics::Shell));
        assert_eq!(classify_name(".envrc"), Some(Semantics::Shell));
        assert_eq!(classify_name("Dockerfile"), Some(Semantics::Literal));
        assert_eq!(classify_name("Dockerfile.tmpl"), Some(Semantics::Literal));
        assert_eq!(classify_name(".env"), Some(Semantics::Literal));
        assert_eq!(classify_name(".env.example"), Some(Semantics::Literal));
        assert_eq!(classify_name("compose.yaml"), Some(Semantics::Shell));
        assert_eq!(classify_name("flake.nix"), Some(Semantics::Shell));
        assert_eq!(classify_name("Justfile"), Some(Semantics::Literal));
        assert_eq!(classify_name("image.png"), None);
        assert_eq!(classify_name("main.rs"), None);
    }
}
