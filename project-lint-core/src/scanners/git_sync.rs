//! Git sync scanner — warns when the local repository is not in sync with
//! its remote upstream.
//!
//! The scanner checks four conditions and emits one issue per condition that
//! applies:
//!
//! - **behind**: the local branch is behind its remote upstream (origin has
//!   commits the local branch does not).
//! - **ahead**: the local branch has commits not yet on the remote upstream
//!   (unpushed work).
//! - **diverged**: the local and remote branches have diverged (both have
//!   unique commits).
//! - **dirty-tree**: the working tree has uncommitted changes.
//!
//! Two branches are checked:
//! 1. The project's main branch (`main` or `master`, configurable) against
//!    `origin/<main>`.
//! 2. The current branch (when different from main) against its configured
//!    upstream.
//!
//! By default the scanner runs `git fetch` before comparing so the
//! remote-tracking refs are current. This can be disabled via
//! `[scanner_config.git_sync] fetch_before_compare = false` for offline or
//! CI environments where network access is unavailable.
//!
//! Repos that are not git repositories, or that have no remote upstream for a
//! given branch, are skipped silently for that branch.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use git2::{Branch, BranchType, Repository};
use std::path::Path;
use tracing::{debug, warn};

/// Default main-branch names checked against `origin/<name>` when no
/// `[scanner_config.git_sync]` override is supplied.
pub const DEFAULT_MAIN_BRANCHES: &[&str] = &["main", "master"];

pub struct GitSyncScanner {
    /// When true, shell out to `git fetch` before comparing local and remote
    /// refs. Defaults to true.
    fetch_before_compare: bool,
    /// Branch names treated as the project's main branch. The first name that
    /// exists locally is checked against `origin/<name>`.
    main_branches: Vec<String>,
}

impl GitSyncScanner {
    pub fn new() -> Self {
        Self {
            fetch_before_compare: true,
            main_branches: DEFAULT_MAIN_BRANCHES
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    pub fn with_config(fetch_before_compare: bool, main_branches: Vec<String>) -> Self {
        let main_branches = if main_branches.is_empty() {
            DEFAULT_MAIN_BRANCHES
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            main_branches
        };
        Self {
            fetch_before_compare,
            main_branches,
        }
    }

    /// Scan `project_path` for git sync violations.
    ///
    /// Returns an empty `Vec` (not an error) when the path is not a git repo.
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);
        let repo = match Repository::open(root) {
            Ok(r) => r,
            Err(e) => {
                debug!("git_sync: not a git repo at {:?}: {}", root, e);
                return Ok(Vec::new());
            }
        };

        if self.fetch_before_compare {
            self.run_fetch(root);
        }

        let mut issues = Vec::new();

        // 1. Check the main branch against origin/<main>.
        if let Some((local_name, local_oid, remote_oid)) = self.resolve_main_branch(&repo) {
            self.check_branch_sync(&repo, &local_name, local_oid, remote_oid, &mut issues);
        } else {
            debug!("git_sync: no main branch found, skipping main-branch sync check");
        }

        // 2. Check the current branch against its upstream, when it differs
        //    from the main branch we already checked.
        if let Some(current_name) = current_branch_name(&repo) {
            let already_checked = self
                .resolve_main_branch(&repo)
                .map(|(name, _, _)| name == current_name)
                .unwrap_or(false);
            if !already_checked {
                if let Some((local_oid, remote_oid)) = current_branch_upstream(&repo) {
                    self.check_branch_sync(
                        &repo,
                        &current_name,
                        local_oid,
                        remote_oid,
                        &mut issues,
                    );
                } else {
                    debug!(
                        "git_sync: current branch '{}' has no upstream, skipping",
                        current_name
                    );
                }
            }
        }

        // 3. Check the working tree for uncommitted changes.
        self.check_dirty_tree(&repo, &mut issues);

        Ok(issues)
    }

    /// Shell out to `git fetch` to refresh remote-tracking refs. Failures are
    /// non-fatal: we log a warning and continue with the refs already present
    /// locally so the scanner still works offline.
    fn run_fetch(&self, root: &Path) {
        debug!("git_sync: running `git fetch` at {:?}", root);
        match std::process::Command::new("git")
            .args(["fetch", "--quiet"])
            .current_dir(root)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    debug!("git_sync: `git fetch` completed");
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        "git_sync: `git fetch` failed (continuing with local refs): {}",
                        stderr.trim()
                    );
                }
            }
            Err(e) => {
                warn!(
                    "git_sync: could not invoke `git fetch` (continuing with local refs): {}",
                    e
                );
            }
        }
    }

    /// Resolve the first main branch (from `self.main_branches`) that exists
    /// locally and has a matching `origin/<name>` remote-tracking ref. Returns
    /// `(local_name, local_oid, remote_oid)`.
    fn resolve_main_branch(&self, repo: &Repository) -> Option<(String, git2::Oid, git2::Oid)> {
        self.main_branches.iter().find_map(|name| {
            let local = repo.find_branch(name, BranchType::Local).ok()?;
            let local_oid = local.get().target()?;
            let remote_name = format!("origin/{}", name);
            let remote = repo.find_branch(&remote_name, BranchType::Remote).ok()?;
            let remote_oid = remote.get().target()?;
            Some((name.clone(), local_oid, remote_oid))
        })
    }

    /// Compare `local_oid` against `remote_oid` for `branch_name` and push
    /// behind / ahead / diverged issues as appropriate.
    fn check_branch_sync(
        &self,
        repo: &Repository,
        branch_name: &str,
        local_oid: git2::Oid,
        remote_oid: git2::Oid,
        issues: &mut Vec<ScannerIssue>,
    ) {
        if local_oid == remote_oid {
            debug!(
                "git_sync: '{}' is in sync with origin/{}",
                branch_name, branch_name
            );
            return;
        }

        let (ahead, behind) = match repo.graph_ahead_behind(local_oid, remote_oid) {
            Ok(ab) => ab,
            Err(e) => {
                warn!(
                    "git_sync: could not compute ahead/behind for '{}': {}",
                    branch_name, e
                );
                return;
            }
        };

        let remote_label = format!("origin/{}", branch_name);

        if ahead > 0 && behind > 0 {
            issues.push(ScannerIssue::new(
                "git-sync-diverged",
                "warning",
                branch_name,
                format!(
                    "branch '{}' has diverged from {} ({} commit(s) ahead, {} behind) — rebase or merge before continuing",
                    branch_name, remote_label, ahead, behind
                ),
            ));
        } else if behind > 0 {
            issues.push(ScannerIssue::new(
                "git-sync-behind",
                "warning",
                branch_name,
                format!(
                    "branch '{}' is {} commit(s) behind {} — run `git pull` to update",
                    branch_name, behind, remote_label
                ),
            ));
        } else if ahead > 0 {
            issues.push(ScannerIssue::new(
                "git-sync-ahead",
                "info",
                branch_name,
                format!(
                    "branch '{}' is {} commit(s) ahead of {} — run `git push` when ready",
                    branch_name, ahead, remote_label
                ),
            ));
        }
    }

    /// Check the working tree and index for uncommitted changes.
    fn check_dirty_tree(&self, repo: &Repository, issues: &mut Vec<ScannerIssue>) {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        opts.exclude_submodules(true);

        let statuses = match repo.statuses(Some(&mut opts)) {
            Ok(s) => s,
            Err(e) => {
                warn!("git_sync: could not read working-tree status: {}", e);
                return;
            }
        };

        if statuses.is_empty() {
            debug!("git_sync: working tree is clean");
            return;
        }

        let dirty = statuses.len();
        debug!("git_sync: working tree has {} dirty entr(y/ies)", dirty);
        issues.push(ScannerIssue::new(
            "git-sync-dirty-tree",
            "warning",
            ".",
            format!(
                "working tree has {} uncommitted change(s) — commit or stash before syncing",
                dirty
            ),
        ));
    }
}

impl Default for GitSyncScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Return the current branch name, or `None` if HEAD is detached or unreadable.
fn current_branch_name(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;
    head.shorthand().map(|s| s.to_string())
}

/// Resolve the current branch's upstream (remote-tracking) ref and return
/// `(local_oid, remote_oid)`. Returns `None` when the current branch has no
/// configured upstream or HEAD is detached.
fn current_branch_upstream(repo: &Repository) -> Option<(git2::Oid, git2::Oid)> {
    let head = repo.head().ok()?;
    let local_oid = head.target()?;
    let shorthand = head.shorthand()?;
    let branch = repo.find_branch(shorthand, BranchType::Local).ok()?;
    let upstream: Branch = branch.upstream().ok()?;
    let remote_oid = upstream.get().target()?;
    Some((local_oid, remote_oid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    /// Set `user.name` / `user.email` so commits work in sandboxed CI.
    fn set_identity(repo: &Repository) {
        if let Ok(mut c) = repo.config() {
            let _ = c.set_str("user.name", "test");
            let _ = c.set_str("user.email", "test@example.com");
        }
    }

    /// Commit a single file to `repo` on HEAD with `message`.
    fn commit_single(repo: &Repository, path: &str, content: &str, message: &str) {
        std::fs::write(repo.workdir().unwrap().join(path), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = head_commit.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
    }

    /// Build a local-only repo with a `main` branch and no remote. Used to
    /// verify the scanner skips gracefully when there is no upstream.
    fn local_only_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        set_identity(&repo);
        commit_single(&repo, "README.md", "# test\n", "init");
        dir
    }

    #[test]
    fn skips_non_git_repo() {
        let dir = TempDir::new().unwrap();
        let scanner = GitSyncScanner::with_config(false, vec![]);
        let issues = scanner.scan(dir.path().to_str().unwrap()).unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn clean_repo_in_sync_emits_no_issues() {
        // A local-only repo has no remote upstream, so no sync issues; the
        // working tree is clean after committing, so no dirty-tree issue.
        let dir = local_only_repo();
        let scanner = GitSyncScanner::with_config(false, vec![]);
        let issues = scanner.scan(dir.path().to_str().unwrap()).unwrap();
        assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
    }

    #[test]
    fn dirty_tree_is_flagged() {
        let dir = local_only_repo();
        // Create an untracked / modified file without committing.
        std::fs::write(dir.path().join("dirty.txt"), "uncommitted\n").unwrap();
        let scanner = GitSyncScanner::with_config(false, vec![]);
        let issues = scanner.scan(dir.path().to_str().unwrap()).unwrap();
        let dirty = issues
            .iter()
            .find(|i| i.rule == "git-sync-dirty-tree")
            .expect("expected a dirty-tree issue");
        assert_eq!(dirty.severity, "warning");
        assert!(dirty.message.contains("uncommitted"));
    }

    #[test]
    fn ahead_of_upstream_is_info() {
        // Build a bare "origin" repo, clone it, make an initial commit, push,
        // then add a second local commit so local is ahead of origin.
        let origin_dir = TempDir::new().unwrap();
        let _origin = Repository::init_bare(origin_dir.path()).unwrap();

        let clone_dir = TempDir::new().unwrap();
        let repo =
            Repository::clone(origin_dir.path().to_str().unwrap(), clone_dir.path()).unwrap();
        set_identity(&repo);
        commit_single(&repo, "README.md", "# init\n", "init");
        // Push to set up upstream tracking.
        push_current(&repo, "origin", "main");

        // Now add a second local commit (ahead of origin).
        commit_single(&repo, "second.txt", "second\n", "second");

        let scanner = GitSyncScanner::with_config(false, vec![]);
        let issues = scanner.scan(clone_dir.path().to_str().unwrap()).unwrap();
        let ahead = issues
            .iter()
            .find(|i| i.rule == "git-sync-ahead")
            .expect("expected an ahead issue");
        assert_eq!(ahead.severity, "info");
        assert!(ahead.message.contains("ahead"));
    }

    #[test]
    fn behind_upstream_is_warning() {
        let origin_dir = TempDir::new().unwrap();
        let _origin = Repository::init_bare(origin_dir.path()).unwrap();

        let clone1_dir = TempDir::new().unwrap();
        let clone1 =
            Repository::clone(origin_dir.path().to_str().unwrap(), clone1_dir.path()).unwrap();
        set_identity(&clone1);
        commit_single(&clone1, "README.md", "# init\n", "init");
        push_current(&clone1, "origin", "main");

        // Second clone gets the init commit.
        let clone2_dir = TempDir::new().unwrap();
        let clone2 =
            Repository::clone(origin_dir.path().to_str().unwrap(), clone2_dir.path()).unwrap();
        set_identity(&clone2);

        // clone1 adds another commit and pushes — clone2 is now behind.
        commit_single(&clone1, "second.txt", "second\n", "second");
        push_current(&clone1, "origin", "main");

        // clone2 fetches (disabled here; we manually update refs) then scans.
        // Manually fetch via git CLI to refresh origin/main in clone2.
        std::process::Command::new("git")
            .args(["fetch", "--quiet"])
            .current_dir(clone2_dir.path())
            .output()
            .unwrap();

        let scanner = GitSyncScanner::with_config(false, vec![]);
        let issues = scanner.scan(clone2_dir.path().to_str().unwrap()).unwrap();
        let behind = issues
            .iter()
            .find(|i| i.rule == "git-sync-behind")
            .expect("expected a behind issue");
        assert_eq!(behind.severity, "warning");
        assert!(behind.message.contains("behind"));
    }

    #[test]
    fn diverged_from_upstream_is_warning() {
        let origin_dir = TempDir::new().unwrap();
        let _origin = Repository::init_bare(origin_dir.path()).unwrap();

        // clone1: initial commit + push.
        let clone1_dir = TempDir::new().unwrap();
        let clone1 =
            Repository::clone(origin_dir.path().to_str().unwrap(), clone1_dir.path()).unwrap();
        set_identity(&clone1);
        commit_single(&clone1, "README.md", "# init\n", "init");
        push_current(&clone1, "origin", "main");

        // clone2: gets the init commit, then adds its own local commit.
        let clone2_dir = TempDir::new().unwrap();
        let clone2 =
            Repository::clone(origin_dir.path().to_str().unwrap(), clone2_dir.path()).unwrap();
        set_identity(&clone2);
        commit_single(&clone2, "local.txt", "local\n", "local-only");

        // clone1: adds a different commit and pushes — now clone2 has diverged.
        commit_single(&clone1, "remote.txt", "remote\n", "remote-only");
        push_current(&clone1, "origin", "main");

        // Refresh clone2's remote-tracking refs.
        std::process::Command::new("git")
            .args(["fetch", "--quiet"])
            .current_dir(clone2_dir.path())
            .output()
            .unwrap();

        let scanner = GitSyncScanner::with_config(false, vec![]);
        let issues = scanner.scan(clone2_dir.path().to_str().unwrap()).unwrap();
        let diverged = issues
            .iter()
            .find(|i| i.rule == "git-sync-diverged")
            .expect("expected a diverged issue");
        assert_eq!(diverged.severity, "warning");
        assert!(diverged.message.contains("diverged"));
        assert!(diverged.message.contains("ahead"));
        assert!(diverged.message.contains("behind"));
    }

    /// Push the current branch to `remote`, setting up upstream tracking.
    /// The branch name is resolved from HEAD so the test works regardless
    /// of the git install's `init.defaultBranch` (`main` vs `master`).
    fn push_current(repo: &Repository, remote: &str, _branch_name: &str) {
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        let tree = commit.tree().unwrap();
        let sig = repo.signature().unwrap();
        let branch_name = head.shorthand().unwrap_or("main").to_string();
        // Build a remote ref update spec: refs/heads/<branch>:refs/heads/<branch>
        let refspec = format!("+refs/heads/{}:refs/heads/{}", branch_name, branch_name);
        let mut remote = repo.find_remote(remote).unwrap();
        remote
            .push(&[refspec], None)
            .unwrap_or_else(|e| panic!("push failed: {}", e));
        // Set up upstream tracking for the local branch.
        if let Ok(mut branch) = repo.find_branch(&branch_name, BranchType::Local) {
            let _ = branch.set_upstream(Some(&format!("origin/{}", branch_name)));
        }
        // Silence unused-tree warning.
        let _ = &tree;
        let _ = &sig;
    }
}
