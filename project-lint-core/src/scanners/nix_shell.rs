//! nix shell scanner — validates `shell.nix` and `default.nix` files for
//! Nix-based dev environments. Enforces four rules:
//!
//! 1. **shell-nix-mkshell** — `shell.nix` should use `pkgs.mkShell` (not raw
//!    derivations).
//! 2. **shell-nix-buildinputs** — `mkShell` should have `buildInputs` or
//!    `packages` defined.
//! 3. **shell-nix-no-floating-nixpkgs** — should not use `import <nixpkgs> {}`
//!    (floating channel); pin to a specific nixpkgs version.
//! 4. **default-nix-not-shell** — `default.nix` should not be a shell
//!    definition (use `shell.nix` for that).

use crate::scanners::ScannerIssue;
use crate::utils::{build_exclusions, is_excluded_rel, walk_project, Result};
use std::path::Path;

pub struct NixShellScanner {
    require_mkshell: bool,
    forbid_floating_nixpkgs: bool,
    excluded: Vec<String>,
}

impl NixShellScanner {
    pub fn new() -> Self {
        Self {
            require_mkshell: true,
            forbid_floating_nixpkgs: true,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_config(require_mkshell: bool, forbid_floating_nixpkgs: bool) -> Self {
        Self {
            require_mkshell,
            forbid_floating_nixpkgs,
            excluded: build_exclusions(&[], false),
        }
    }

    pub fn with_exclusions(
        require_mkshell: bool,
        forbid_floating_nixpkgs: bool,
        excluded: Vec<String>,
    ) -> Self {
        Self {
            require_mkshell,
            forbid_floating_nixpkgs,
            excluded,
        }
    }

    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let mut issues = Vec::new();

        for entry in walk_project(root, &self.excluded, 4).filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "shell.nix" && name != "default.nix" {
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
            issues.extend(self.scan_nix_file(path, &rel, &name));
        }

        Ok(issues)
    }

    fn scan_nix_file(&self, path: &Path, rel: &str, name: &str) -> Vec<ScannerIssue> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut issues = Vec::new();
        let has_mkshell = content.contains("mkShell");
        let has_buildinputs = content.contains("buildInputs");
        let has_packages = content.contains("packages");
        let has_floating = content.contains("import <nixpkgs>");

        if name == "shell.nix" {
            if self.require_mkshell && !has_mkshell {
                issues.push(ScannerIssue::new(
                    "shell-nix-mkshell",
                    "warning",
                    rel,
                    "shell.nix should use pkgs.mkShell (not raw derivations)",
                ));
            }
            if has_mkshell && !has_buildinputs && !has_packages {
                issues.push(ScannerIssue::new(
                    "shell-nix-buildinputs",
                    "warning",
                    rel,
                    "mkShell should have buildInputs or packages defined",
                ));
            }
            if self.forbid_floating_nixpkgs && has_floating {
                issues.push(ScannerIssue::new(
                    "shell-nix-no-floating-nixpkgs",
                    "warning",
                    rel,
                    "shell.nix uses 'import <nixpkgs> {}' (floating channel) — pin to a specific nixpkgs version",
                ));
            }
        } else if name == "default.nix" {
            if has_mkshell {
                issues.push(ScannerIssue::new(
                    "default-nix-not-shell",
                    "info",
                    rel,
                    "default.nix should not be a shell definition — use shell.nix for that",
                ));
            }
            if self.forbid_floating_nixpkgs && has_floating {
                issues.push(ScannerIssue::new(
                    "shell-nix-no-floating-nixpkgs",
                    "warning",
                    rel,
                    "default.nix uses 'import <nixpkgs> {}' (floating channel) — pin to a specific nixpkgs version",
                ));
            }
        }

        issues
    }
}

impl Default for NixShellScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clean_shell_nix_has_no_issues() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("shell.nix"),
            "{ pkgs ? import (fetchTarball \"https://github.com/NixOS/nixpkgs/archive/nixos-24.05.tar.gz\") {} }:\npkgs.mkShell {\n  buildInputs = [ pkgs.go ];\n}\n",
        )?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
        Ok(())
    }

    #[test]
    fn flags_shell_nix_without_mkshell() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("shell.nix"),
            "{ pkgs ? import (fetchTarball \"https://github.com/NixOS/nixpkgs/archive/nixos-24.05.tar.gz\") {} }:\nstdenv.mkDerivation {\n  name = \"shell\";\n}\n",
        )?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "shell-nix-mkshell"),
            "expected shell-nix-mkshell warning"
        );
        Ok(())
    }

    #[test]
    fn flags_mkshell_without_buildinputs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("shell.nix"),
            "{ pkgs ? import (fetchTarball \"https://github.com/NixOS/nixpkgs/archive/nixos-24.05.tar.gz\") {} }:\npkgs.mkShell {\n  name = \"shell\";\n}\n",
        )?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "shell-nix-buildinputs"),
            "expected shell-nix-buildinputs warning"
        );
        Ok(())
    }

    #[test]
    fn flags_floating_nixpkgs() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("shell.nix"),
            "{ pkgs ? import <nixpkgs> {} }:\npkgs.mkShell {\n  buildInputs = [ pkgs.go ];\n}\n",
        )?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues
                .iter()
                .any(|i| i.rule == "shell-nix-no-floating-nixpkgs"),
            "expected shell-nix-no-floating-nixpkgs warning"
        );
        Ok(())
    }

    #[test]
    fn flags_default_nix_as_shell() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("default.nix"),
            "{ pkgs ? import (fetchTarball \"https://github.com/NixOS/nixpkgs/archive/nixos-24.05.tar.gz\") {} }:\npkgs.mkShell {\n  buildInputs = [ pkgs.go ];\n}\n",
        )?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "default-nix-not-shell"),
            "expected default-nix-not-shell info"
        );
        Ok(())
    }

    #[test]
    fn silent_on_repo_without_nix_files() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("README.md"), "# no nix here\n")?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn empty_shell_nix_emits_mkshell_warning() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(dir.path().join("shell.nix"), "")?;
        let scanner = NixShellScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(
            issues.iter().any(|i| i.rule == "shell-nix-mkshell"),
            "empty shell.nix should warn about missing mkShell"
        );
        Ok(())
    }

    #[test]
    fn config_can_disable_mkshell_check() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join("shell.nix"),
            "stdenv.mkDerivation { name = \"x\"; }\n",
        )?;
        let scanner = NixShellScanner::with_config(false, false);
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }
}
