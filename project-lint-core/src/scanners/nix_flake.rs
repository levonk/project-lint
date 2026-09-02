//! nix flake scanner — validates `flake.nix` and `flake.lock` files for
//! Nix flake-based dev environments. Enforces nine rules:
//!
//! #### flake.nix rules
//! 1. **flake-inputs-have-urls** — every input in `inputs.{}` must have a
//!    `url` field (explicit or shorthand).
//! 2. **flake-inputs-pinned** — inputs should pin to a specific ref/rev, not
//!    floating (`github:owner/repo` without ref).
//! 3. **flake-nixpkgs-not-floating** — `nixpkgs` input should not use
//!    `nixpkgs-unstable` for production projects.
//! 4. **flake-outputs-function** — `outputs` must be a function.
//! 5. **flake-has-description** — flake should have a top-level `description`.
//! 6. **flake-no-flake-false** — must not set `flake = false` in inputs unless
//!    it's a non-flake input.
//!
//! #### flake.lock rules
//! 7. **flake-lock-present** — if `flake.nix` exists, `flake.lock` must exist.
//! 8. **flake-lock-fresh** — all inputs in `flake.nix` should have entries in
//!    `flake.lock`.
//! 9. **flake-lock-nar-hash-present** — every node in `flake.lock` must have a
//!    `narHash` or `narinfo`.
//!
//! Nix is not trivially parseable by serde, so `flake.nix` is analyzed with
//! regex to extract `inputs` and `outputs` blocks. `flake.lock` is parsed as
//! JSON using `serde_json`.

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use serde_json::Value;
use std::path::Path;

pub struct NixFlakeScanner {
    require_stable_nixpkgs: bool,
    check_lock_freshness: bool,
    excluded: Vec<String>,
}

impl NixFlakeScanner {
    pub fn new() -> Self {
        Self {
            require_stable_nixpkgs: false,
            check_lock_freshness: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_stable_nixpkgs: bool, check_lock_freshness: bool) -> Self {
        Self {
            require_stable_nixpkgs,
            check_lock_freshness,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_stable_nixpkgs: bool,
        check_lock_freshness: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_stable_nixpkgs,
            check_lock_freshness,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        let mut flake_nix_paths: Vec<(String, std::path::PathBuf)> = Vec::new();
        let mut flake_lock_paths: Vec<(String, std::path::PathBuf)> = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "flake.nix" && name != "flake.lock" {
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
            if name == "flake.nix" {
                flake_nix_paths.push((rel.clone(), path.to_path_buf()));
            } else {
                flake_lock_paths.push((rel.clone(), path.to_path_buf()));
            }
        }

        for (rel, path) in &flake_nix_paths {
            issues.extend(self.scan_flake_nix(path, rel, root));
        }
        for (rel, path) in &flake_lock_paths {
            issues.extend(self.scan_flake_lock(path, rel));
        }

        Ok(issues)
    }

    fn scan_flake_nix(&self, path: &Path, rel: &str, root: &Path) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();

        if !content.contains("description") {
            issues.push(ScannerIssue::new(
                "flake-has-description",
                "info",
                rel,
                "flake.nix should have a top-level 'description' field",
            ));
        }

        let outputs_is_function = is_outputs_a_function(&content);
        if !outputs_is_function {
            issues.push(ScannerIssue::new(
                "flake-outputs-function",
                "error",
                rel,
                "outputs must be a function (taking self and inputs)",
            ));
        }

        let inputs = extract_inputs_block(&content);
        let input_names = parse_input_names(&inputs);
        for name in &input_names {
            if name == "flake" {
                continue;
            }
            let has_url = input_has_url(&inputs, name);
            if !has_url {
                issues.push(ScannerIssue::new(
                    "flake-inputs-have-urls",
                    "error",
                    rel,
                    format!(
                        "input '{}' must have a 'url' field (explicit or shorthand)",
                        name
                    ),
                ));
            }
            if has_url && !input_is_pinned(&inputs, name) {
                issues.push(ScannerIssue::new(
                    "flake-inputs-pinned",
                    "warning",
                    rel,
                    format!(
                        "input '{}' should pin to a specific ref/rev, not floating",
                        name
                    ),
                ));
            }
            if self.require_stable_nixpkgs && name == "nixpkgs" {
                if input_uses_unstable(&inputs, name) {
                    issues.push(ScannerIssue::new(
                        "flake-nixpkgs-not-floating",
                        "info",
                        rel,
                        "nixpkgs input uses nixpkgs-unstable — recommend a stable channel (e.g. nixos-24.05)",
                    ));
                }
            }
            if input_has_flake_false(&inputs, name) {
                issues.push(ScannerIssue::new(
                    "flake-no-flake-false",
                    "warning",
                    rel,
                    format!(
                        "input '{}' sets flake = false — ensure it is a non-flake input",
                        name
                    ),
                ));
            }
        }

        let lock_path = path.parent().unwrap_or(root).join("flake.lock");
        if !lock_path.exists() {
            issues.push(ScannerIssue::new(
                "flake-lock-present",
                "error",
                rel,
                "flake.nix exists but flake.lock is missing — run 'nix flake lock' and commit it",
            ));
        } else if self.check_lock_freshness {
            if let Ok(lock_content) = std::fs::read_to_string(&lock_path) {
                let lock_inputs = parse_lock_inputs(&lock_content);
                for name in &input_names {
                    if name == "flake" {
                        continue;
                    }
                    if !lock_inputs.iter().any(|n| n == name) {
                        issues.push(ScannerIssue::new(
                            "flake-lock-fresh",
                            "warning",
                            rel,
                            format!(
                                "input '{}' in flake.nix has no entry in flake.lock — run 'nix flake update'",
                                name
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn scan_flake_lock(&self, path: &Path, rel: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let json: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                return vec![ScannerIssue::new(
                    "flake-lock-parse",
                    "error",
                    rel,
                    format!("flake.lock is not valid JSON: {}", e),
                )]
            }
        };
        let mut issues = Vec::new();
        if let Some(nodes) = json.get("nodes").and_then(|n| n.as_object()) {
            for (node_name, node_val) in nodes {
                if node_name == "root" {
                    continue;
                }
                let has_narhash = node_val.get("narHash").is_some();
                let has_narinfo = node_val.get("narinfo").is_some();
                if !has_narhash && !has_narinfo {
                    issues.push(ScannerIssue::new(
                        "flake-lock-nar-hash-present",
                        "error",
                        rel,
                        format!(
                            "node '{}' in flake.lock is missing narHash/narinfo",
                            node_name
                        ),
                    ));
                }
            }
        }
        issues
    }
}

fn is_outputs_a_function(content: &str) -> bool {
    let re = regex::Regex::new(r"outputs\s*=\s*[^;]*?:\s*").ok();
    re.map(|r| r.is_match(content)).unwrap_or(false)
}

fn extract_inputs_block(content: &str) -> String {
    if let Some(start) = content.find("inputs") {
        let rest = &content[start..];
        if let Some(brace_start) = rest.find('{') {
            let mut depth = 0i32;
            let mut end = brace_start;
            for (i, ch) in rest[brace_start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = brace_start + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            return rest[brace_start..=end].to_string();
        }
    }
    String::new()
}

fn parse_input_names(inputs_block: &str) -> Vec<String> {
    let mut names = Vec::new();
    let long_re = regex::Regex::new(r"([a-zA-Z_][a-zA-Z0-9_-]*)\s*\.(?:url|flake|inputs)\b").ok();
    if let Some(ref r) = long_re {
        for caps in r.captures_iter(inputs_block) {
            let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if !name.is_empty() && !names.contains(&name.to_string()) {
                names.push(name.to_string());
            }
        }
    }
    let short_re =
        regex::Regex::new(r#"(?:[{;]\s*)([a-zA-Z_][a-zA-Z0-9_-]*)\s*=\s*"(?:github:|git\+|path:)"#)
            .ok();
    if let Some(ref r) = short_re {
        for caps in r.captures_iter(inputs_block) {
            let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if !name.is_empty() && !names.contains(&name.to_string()) {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn input_has_url(inputs_block: &str, name: &str) -> bool {
    let re = regex::Regex::new(&format!(
        r"(?s){}\s*[.=].*?(?:url\s*=|github:|git\+|path:|tarball)",
        regex::escape(name)
    ))
    .ok();
    re.map(|r| r.is_match(inputs_block)).unwrap_or(false)
}

fn input_is_pinned(inputs_block: &str, name: &str) -> bool {
    let re = regex::Regex::new(&format!(
        r"(?s){}\s*[.=].*?(?:ref\s*=|rev\s*=|#)",
        regex::escape(name)
    ))
    .ok();
    if re.map(|r| r.is_match(inputs_block)).unwrap_or(false) {
        return true;
    }
    let github_re = regex::Regex::new(&format!(
        r"(?s){}\s*[.=].*?github:[^/]+/[^/]+/[^/]+",
        regex::escape(name)
    ))
    .ok();
    github_re.map(|r| r.is_match(inputs_block)).unwrap_or(false)
}

fn input_uses_unstable(inputs_block: &str, name: &str) -> bool {
    let re = regex::Regex::new(&format!(
        r"(?s){}\s*[.=].*?nixpkgs-unstable",
        regex::escape(name)
    ))
    .ok();
    re.map(|r| r.is_match(inputs_block)).unwrap_or(false)
}

fn input_has_flake_false(inputs_block: &str, name: &str) -> bool {
    let re = regex::Regex::new(&format!(
        r"(?s){}\s*[.=].*?flake\s*=\s*false",
        regex::escape(name)
    ))
    .ok();
    re.map(|r| r.is_match(inputs_block)).unwrap_or(false)
}

fn parse_lock_inputs(lock_content: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(json) = serde_json::from_str::<Value>(lock_content) {
        if let Some(nodes) = json.get("nodes").and_then(|n| n.as_object()) {
            for name in nodes.keys() {
                if name != "root" {
                    names.push(name.clone());
                }
            }
        }
    }
    names
}

impl Default for NixFlakeScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn clean_flake_nix() -> &'static str {
        r#"{
  description = "My project flake";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils/main";
  };
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system}; in
      { devShells.default = pkgs.mkShell { buildInputs = [ pkgs.go ]; }; });
}
"#
    }

    fn clean_flake_lock() -> &'static str {
        r#"{
  "nodes": {
    "root": {},
    "nixpkgs": { "narHash": "sha256-abc" },
    "flake-utils": { "narHash": "sha256-def" }
  }
}
"#
    }

    #[test]
    fn clean_flake_with_lock_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("flake.nix"), clean_flake_nix())?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_missing_lock() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("flake.nix"), clean_flake_nix())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-lock-present"),
            "expected flake-lock-present error"
        );
        Ok(())
    }

    #[test]
    fn flags_missing_description() -> Result<()> {
        let dir = TempDir::new()?;
        let flake = r#"{
  inputs = { nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05"; };
  outputs = { self, nixpkgs }: { };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake)?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-has-description"),
            "expected flake-has-description info"
        );
        Ok(())
    }

    #[test]
    fn flags_outputs_not_a_function() -> Result<()> {
        let dir = TempDir::new()?;
        let flake = r#"{ description = "x"; inputs = {}; outputs = { }; }
"#;
        std::fs::write(dir.path().join("flake.nix"), flake)?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-outputs-function"),
            "expected flake-outputs-function error"
        );
        Ok(())
    }

    #[test]
    fn flags_floating_input() -> Result<()> {
        let dir = TempDir::new()?;
        let flake = r#"{ description = "x";
  inputs = { nixpkgs.url = "github:NixOS/nixpkgs"; };
  outputs = { self, nixpkgs }: { };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake)?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-inputs-pinned"),
            "expected flake-inputs-pinned warning for floating nixpkgs"
        );
        Ok(())
    }

    #[test]
    fn flags_unstable_nixpkgs_when_required() -> Result<()> {
        let dir = TempDir::new()?;
        let flake = r#"{ description = "x";
  inputs = { nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable"; };
  outputs = { self, nixpkgs }: { };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake)?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::with_config(true, true);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues
                .iter()
                .any(|i| i.rule == "flake-nixpkgs-not-floating"),
            "expected flake-nixpkgs-not-floating info"
        );
        Ok(())
    }

    #[test]
    fn flags_flake_false_in_inputs() -> Result<()> {
        let dir = TempDir::new()?;
        let flake = r#"{ description = "x";
  inputs = { nonflake.url = "github:owner/repo"; nonflake.flake = false; };
  outputs = { self }: { };
}
"#;
        std::fs::write(dir.path().join("flake.nix"), flake)?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-no-flake-false"),
            "expected flake-no-flake-false warning"
        );
        Ok(())
    }

    #[test]
    fn flags_lock_node_missing_narhash() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("flake.nix"), clean_flake_nix())?;
        let lock = r#"{
  "nodes": {
    "root": {},
    "nixpkgs": { "narHash": "sha256-abc" },
    "flake-utils": {}
  }
}
"#;
        std::fs::write(dir.path().join("flake.lock"), lock)?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues
                .iter()
                .any(|i| i.rule == "flake-lock-nar-hash-present"),
            "expected flake-lock-nar-hash-present error"
        );
        Ok(())
    }

    #[test]
    fn flags_lock_not_fresh() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("flake.nix"), clean_flake_nix())?;
        let lock = r#"{ "nodes": { "root": {}, "nixpkgs": { "narHash": "sha256-abc" } } }
"#;
        std::fs::write(dir.path().join("flake.lock"), lock)?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-lock-fresh"),
            "expected flake-lock-fresh warning for missing flake-utils in lock"
        );
        Ok(())
    }

    #[test]
    fn silent_on_repo_without_flake_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# no flake here\n")?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn flags_invalid_lock_json() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("flake.nix"), clean_flake_nix())?;
        std::fs::write(dir.path().join("flake.lock"), "{not valid json")?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "flake-lock-parse"),
            "expected flake-lock-parse error"
        );
        Ok(())
    }

    #[test]
    fn empty_flake_nix_emits_description_and_outputs_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("flake.nix"), "")?;
        std::fs::write(dir.path().join("flake.lock"), clean_flake_lock())?;
        let scanner = NixFlakeScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.iter().any(|i| i.rule == "flake-has-description"));
        assert!(issues.iter().any(|i| i.rule == "flake-outputs-function"));
        Ok(())
    }
}
