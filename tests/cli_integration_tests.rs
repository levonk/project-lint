//! CLI integration tests using `assert_cmd` + `predicates`.
//!
//! Exercises the compiled `project-lint` binary: `--version`, `--help`,
//! subcommand help, `lint` on an empty project, and error paths for unknown
//! subcommands. These are true end-to-end CLI checks (exit codes + stdout).

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn cli_version_prints_and_exits_zero() {
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("project-lint").and(predicate::str::contains("0.1.0")));
}

#[test]
fn cli_help_prints_subcommands_and_exits_zero() {
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("lint"))
        .stdout(predicate::str::contains("hook"))
        .stdout(predicate::str::contains("policy"));
}

#[test]
fn cli_lint_subcommand_help_exits_zero() {
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.args(["lint", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--fix"))
        .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn cli_lint_on_empty_project_exits_zero() {
    let dir = TempDir::new().expect("tempdir");
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.args(["lint", "--path"])
        .arg(dir.path().to_string_lossy().to_string())
        .assert()
        .success();
}

#[test]
fn cli_hook_subcommand_help_exits_zero() {
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.args(["hook", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--source"));
}

#[test]
fn cli_policy_subcommand_help_exits_zero() {
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.args(["policy", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("export"));
}

#[test]
fn cli_unknown_subcommand_exits_nonzero() {
    let mut cmd = Command::cargo_bin("project-lint").expect("binary");
    cmd.args(["no-such-subcommand"]).assert().failure();
}
