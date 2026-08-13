//! Submodule integrity scanner — detects when a git submodule (declared in
//! `.gitmodules`) is tracked in the parent repo's HEAD tree as a regular
//! directory (`040000 tree`) instead of a gitlink (`160000 commit`).
//!
//! This is the same class of bug that bit the infrahub repo on commit `9704d5e`
//! (Aug 9 2026): an accidental `git add` on a submodule directory converted the
//! gitlink into a tree, causing 28+ submodule files to be tracked directly in
//! the parent repo. The bug went undetected for 17 commits.
//!
//! The scanner checks the **HEAD tree** by default (catches already-committed
//! corruption). It does not auto-fix — the fix (`git rm --cached -r` +
//! `git update-index --add --cacheinfo`) is destructive to the index and must
//! be run manually. The exact fix command is included in the issue message.
//!
//! Repos without a `.gitmodules` file are skipped silently.

use crate::scanners::ScannerIssue;
use crate::utils::Result;
use git2::{ObjectType, Repository, Tree, TreeEntry};
use std::path::Path;
use tracing::{debug, warn};

pub struct SubmoduleIntegrityScanner {
    /// When true, also inspect the staged index in addition to HEAD. Defaults
    /// to false (HEAD-only) so the scanner acts as a "lint existing state" tool
    /// rather than a pre-commit gate. The infrahub pre-commit hook remains the
    /// first-line defense for staged-index checks.
    check_index: bool,
}

impl SubmoduleIntegrityScanner {
    pub fn new() -> Self {
        Self { check_index: false }
    }

    pub fn with_config(check_index: bool) -> Self {
        Self { check_index }
    }

    /// Scan `project_path` for submodule integrity violations.
    ///
    /// Returns an empty `Vec` (not an error) when:
    /// - the path is not a git repo
    /// - the repo has no `.gitmodules` file
    /// - the repo has no HEAD (e.g. a brand-new repo with zero commits)
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        let root = Path::new(project_path);

        // Skip silently when there is no .gitmodules — this scanner is a no-op
        // for repos that don't use submodules.
        if !root.join(".gitmodules").exists() {
            debug!("submodule_integrity: no .gitmodules at {:?}", root);
            return Ok(Vec::new());
        }

        let repo = match Repository::open(root) {
            Ok(r) => r,
            Err(e) => {
                debug!("submodule_integrity: not a git repo at {:?}: {}", root, e);
                return Ok(Vec::new());
            }
        };

        let mut issues = Vec::new();

        // HEAD-tree check (default).
        if let Ok(head_tree) = head_tree(&repo) {
            issues.extend(self.check_tree(&repo, &head_tree, "HEAD"));
        } else {
            debug!("submodule_integrity: repo has no HEAD tree, skipping HEAD check");
        }

        // Optional staged-index check.
        if self.check_index {
            if let Ok(index) = repo.index() {
                issues.extend(self.check_index(&repo, &index));
            } else {
                warn!("submodule_integrity: could not read index for staged check");
            }
        }

        Ok(issues)
    }

    /// Check every declared submodule path against a single tree (HEAD or a
    /// tree built from the staged index).
    fn check_tree(&self, repo: &Repository, tree: &Tree, label: &str) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();

        let submodules = match repo.submodules() {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "submodule_integrity: failed to enumerate submodules for {}: {}",
                    label, e
                );
                return issues;
            }
        };

        for sm in submodules {
            let sm_path = sm.path().to_path_buf();
            let sm_path_str = sm_path.to_string_lossy().to_string();

            // Resolve the submodule's expected gitlink SHA. `head_id` is the
            // OID recorded by the superproject (the gitlink target). When the
            // entry has been corrupted into a tree, this may still be present
            // in the submodule's config; otherwise we fall back to the
            // submodule's own HEAD.
            let sha = sm
                .head_id()
                .map(|o| o.to_string())
                .or_else(|| submodule_working_head_sha(repo, &sm_path));

            match tree.get_path(&sm_path) {
                Ok(entry) => match entry.kind() {
                    Some(ObjectType::Commit) => {
                        // Correct: gitlink/commit. Nothing to flag.
                        debug!(
                            "submodule_integrity: {} '{}' is a gitlink (OK)",
                            label, sm_path_str
                        );
                    }
                    Some(ObjectType::Tree) => {
                        // VIOLATION: submodule converted to a regular tree.
                        let leaked = list_tree_files(repo, tree, &sm_path);
                        issues.push(violation_tree_instead_of_gitlink(
                            &sm_path_str,
                            sha.as_deref(),
                            &leaked,
                        ));
                    }
                    other => {
                        // Any other mode is unexpected for a submodule path.
                        issues.push(violation_unexpected_mode(
                            &sm_path_str,
                            other,
                            sha.as_deref(),
                        ));
                    }
                },
                Err(_) => {
                    // The submodule path is not present as a direct entry. It
                    // may still have leaked files tracked under it (gitlink
                    // missing, files present).
                    let leaked = list_tree_files(repo, tree, &sm_path);
                    if !leaked.is_empty() {
                        issues.push(violation_gitlink_missing_with_leaked_files(
                            &sm_path_str,
                            sha.as_deref(),
                            &leaked,
                        ));
                    } else {
                        // No gitlink and no leaked files: submodule declared in
                        // .gitmodules but never added to the superproject. This
                        // is a different problem (incomplete setup) — flag at
                        // info severity so users know but it isn't an error.
                        issues.push(ScannerIssue::new(
                            "submodule-not-tracked",
                            "info",
                            &sm_path_str,
                            format!(
                                "submodule '{}' is declared in .gitmodules but not present in {}",
                                sm_path_str, label
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    /// Check the staged index directly. Index entries for files under a
    /// submodule path indicate the submodule has been expanded into individual
    /// tracked files.
    fn check_index(&self, repo: &Repository, index: &git2::Index) -> Vec<ScannerIssue> {
        let mut issues = Vec::new();

        let submodules = match repo.submodules() {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "submodule_integrity: failed to enumerate submodules for index check: {}",
                    e
                );
                return issues;
            }
        };

        for sm in submodules {
            let sm_path = sm.path().to_path_buf();
            let sm_path_str = sm_path.to_string_lossy().to_string();
            let prefix = format!("{}/", sm_path_str);

            // Look for a direct gitlink entry at the submodule path.
            let has_gitlink = index
                .get_path(&sm_path, 0)
                .is_some_and(|e| e.mode == 0o160000);

            if has_gitlink {
                continue;
            }

            // Collect any leaked file entries under the submodule path.
            let leaked: Vec<String> = index
                .iter()
                .map(|e| String::from_utf8_lossy(&e.path).to_string())
                .filter(|p| p == &sm_path_str || p.starts_with(&prefix))
                .collect();

            if !leaked.is_empty() {
                let sha = sm
                    .head_id()
                    .map(|o| o.to_string())
                    .or_else(|| submodule_working_head_sha(repo, &sm_path));
                issues.push(violation_gitlink_missing_with_leaked_files(
                    &sm_path_str,
                    sha.as_deref(),
                    &leaked,
                ));
            }
        }

        issues
    }
}

impl Default for SubmoduleIntegrityScanner {
    fn default() -> Self {
        Self::new()
    }
}

// --- Issue builders --------------------------------------------------------

fn violation_tree_instead_of_gitlink(
    sm_path: &str,
    sha: Option<&str>,
    leaked: &[String],
) -> ScannerIssue {
    let leaked_list = format_file_list(leaked, 8);
    ScannerIssue::new(
        "submodule-tree-instead-of-gitlink",
        "error",
        sm_path,
        format!(
            "submodule '{}' is tracked as a directory (040000 tree) instead of a gitlink (160000 commit). \
             {} leaked file(s) tracked directly in the parent repo. \
             Fix: {}{}",
            sm_path,
            leaked.len(),
            fix_command(sm_path, sha),
            leaked_list
        ),
    )
}

fn violation_gitlink_missing_with_leaked_files(
    sm_path: &str,
    sha: Option<&str>,
    leaked: &[String],
) -> ScannerIssue {
    let leaked_list = format_file_list(leaked, 8);
    ScannerIssue::new(
        "submodule-gitlink-missing",
        "error",
        sm_path,
        format!(
            "submodule '{}' has no gitlink entry but {} file(s) are tracked directly under it in the parent repo. \
             Fix: {}{}",
            sm_path,
            leaked.len(),
            fix_command(sm_path, sha),
            leaked_list
        ),
    )
}

fn violation_unexpected_mode(
    sm_path: &str,
    kind: Option<ObjectType>,
    sha: Option<&str>,
) -> ScannerIssue {
    ScannerIssue::new(
        "submodule-unexpected-mode",
        "error",
        sm_path,
        format!(
            "submodule '{}' has unexpected tree entry kind {:?} (expected gitlink/commit 0o160000). \
             Fix: {}",
            sm_path,
            kind,
            fix_command(sm_path, sha)
        ),
    )
}

// --- Helpers ---------------------------------------------------------------

/// Resolve the HEAD tree of `repo`, or an error if there is no HEAD yet.
fn head_tree(repo: &Repository) -> Result<Tree<'_>> {
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    Ok(commit.tree()?)
}

/// Best-effort: open the submodule's own working repo and read its HEAD SHA.
/// Returns `None` if the submodule working copy or its HEAD is unavailable.
fn submodule_working_head_sha(parent: &Repository, sm_path: &Path) -> Option<String> {
    let workdir = parent.workdir()?;
    let sm_workdir = workdir.join(sm_path);
    let sm_repo = Repository::open(&sm_workdir).ok()?;
    let head = sm_repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    Some(commit.id().to_string())
}

/// Build the manual fix command for a corrupted submodule. The SHA is filled
/// in when known; otherwise a placeholder is left for the user to complete.
fn fix_command(sm_path: &str, sha: Option<&str>) -> String {
    let sha = sha.unwrap_or("<submodule-sha>");
    format!(
        "git rm --cached -r '{}' && git update-index --add --cacheinfo 160000,{},'{}'",
        sm_path, sha, sm_path
    )
}

/// Walk `tree` for every file entry under `prefix` (recursively) and return
/// their paths relative to the tree root. Used to enumerate the leaked files
/// that were accidentally tracked in place of the submodule gitlink.
fn list_tree_files(repo: &Repository, tree: &Tree, prefix: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let prefix_str = prefix.to_string_lossy().to_string();
    walk_tree(repo, tree, "", &prefix_str, &mut out);
    out.sort();
    out
}

fn walk_tree(
    repo: &Repository,
    tree: &Tree,
    current_prefix: &str,
    target_prefix: &str,
    out: &mut Vec<String>,
) {
    for i in 0..tree.len() {
        let entry = match tree.get(i) {
            Some(e) => e,
            None => continue,
        };
        let name = entry.name().unwrap_or("").to_string();
        let full = if current_prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", current_prefix, name)
        };

        if full == target_prefix {
            // We're inside the target subtree — enumerate everything beneath.
            if entry.kind() == Some(ObjectType::Tree) {
                if let Ok(sub) = entry_to_tree(repo, &entry) {
                    enumerate_all(repo, &sub, &full, out);
                }
            } else {
                out.push(full.clone());
            }
            continue;
        }

        // Recurse into subtrees that contain the target prefix.
        if entry.kind() == Some(ObjectType::Tree)
            && (target_prefix.starts_with(&format!("{}/", full)) || full.is_empty())
        {
            if let Ok(sub) = entry_to_tree(repo, &entry) {
                walk_tree(repo, &sub, &full, target_prefix, out);
            }
        }
    }
}

fn enumerate_all(repo: &Repository, tree: &Tree, prefix: &str, out: &mut Vec<String>) {
    for i in 0..tree.len() {
        let entry = match tree.get(i) {
            Some(e) => e,
            None => continue,
        };
        let name = entry.name().unwrap_or("").to_string();
        let full = format!("{}/{}", prefix, name);
        if entry.kind() == Some(ObjectType::Tree) {
            if let Ok(sub) = entry_to_tree(repo, &entry) {
                enumerate_all(repo, &sub, &full, out);
            }
        } else {
            out.push(full);
        }
    }
}

fn entry_to_tree<'a>(repo: &'a Repository, entry: &TreeEntry) -> Result<Tree<'a>> {
    let obj = entry.to_object(repo)?;
    Ok(obj.peel_to_tree()?)
}

/// Render a compact, capped list of leaked file paths for inclusion in the
/// issue message.
fn format_file_list(files: &[String], cap: usize) -> String {
    if files.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = files.iter().take(cap).map(String::as_str).collect();
    let mut s = format!("\n  Leaked files: {}", shown.join(", "));
    if files.len() > cap {
        s.push_str(&format!(" ... ({} more)", files.len() - cap));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexEntry, IndexTime, Repository};
    use tempfile::TempDir;

    /// Set `user.name` / `user.email` on a repo so commits work in CI/sandbox
    /// environments that don't have global git identity configured.
    fn set_identity(repo: &Repository) {
        let _ = repo.config().and_then(|mut c| {
            c.set_str("user.name", "test").ok();
            c.set_str("user.email", "test@example.com").ok();
            Ok(())
        });
    }

    /// Commit `path` (a single file already on disk) in `repo` with `message`.
    fn commit_single(repo: &Repository, path: &str, message: &str) {
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

    /// Simulate the infrahub accident: remove the submodule gitlink from the
    /// parent index and add a file from inside the submodule directory as a
    /// regular tracked file. This mirrors what `git add levonk/` does when the
    /// gitlink has already been removed.
    ///
    /// We use libgit2 directly because `git add --force` on a path inside a
    /// registered submodule is silently ignored by the git CLI (it still
    /// treats the path as a submodule boundary).
    fn leak_submodule_file(repo: &Repository, file_rel_path: &str, content: &[u8]) {
        // Create a blob in the parent object DB and add an index entry for the
        // file under the submodule path, with a regular-file mode.
        let blob_oid = repo.blob(content).unwrap();
        let mut index = repo.index().unwrap();
        let entry = IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: 0o100644,
            uid: 0,
            gid: 0,
            file_size: content.len() as u32,
            id: blob_oid,
            flags: 0,
            flags_extended: 0,
            path: file_rel_path.as_bytes().to_vec(),
        };
        index.add(&entry).unwrap();
        index.write().unwrap();
    }

    /// Remove the gitlink entry for `path` from the parent index.
    fn remove_gitlink(repo: &Repository, path: &str) {
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new(path)).unwrap();
        index.write().unwrap();
    }

    /// Create a parent repo with a single commit containing a `.gitmodules`
    /// file and a properly-tracked submodule gitlink pointing at a real
    /// submodule working repo. The submodule has its own initial commit so
    /// `repo.submodules()` enumerates it and `head_id()` resolves.
    fn setup_parent_with_submodule() -> TempDir {
        let dir = TempDir::new().unwrap();
        let parent = Repository::init(dir.path()).unwrap();
        set_identity(&parent);

        // Create the submodule working dir + its own repo.
        let sm_path = dir.path().join("levonk");
        std::fs::create_dir_all(&sm_path).unwrap();
        let sm_repo = Repository::init(&sm_path).unwrap();
        set_identity(&sm_repo);
        std::fs::write(sm_path.join("README.md"), "# sub\n").unwrap();
        commit_single(&sm_repo, "README.md", "sub init");

        // Write .gitmodules in the parent and commit it (so the parent has a
        // HEAD before we register the submodule gitlink).
        let gitmodules = "[submodule \"levonk\"]\n\tpath = levonk\n\turl = ../levonk.git\n";
        std::fs::write(dir.path().join(".gitmodules"), gitmodules).unwrap();
        commit_single(&parent, ".gitmodules", "parent: add .gitmodules");

        // Add the submodule gitlink to the parent index by constructing an
        // IndexEntry with mode 0o160000 pointing at the submodule's HEAD OID.
        let sm_head = sm_repo.head().unwrap().peel_to_commit().unwrap();
        let mut index = parent.index().unwrap();
        let entry = IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: 0o160000,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: sm_head.id(),
            flags: 0,
            flags_extended: 0,
            path: b"levonk".to_vec(),
        };
        index.add(&entry).unwrap();
        index.write().unwrap();

        // Commit the parent with the gitlink, using the previous HEAD as the
        // parent (required by libgit2 — passing `&[]` would create a root
        // commit and fail with "current tip is not the first parent").
        let tree_oid = index.write_tree().unwrap();
        let tree = parent.find_tree(tree_oid).unwrap();
        let sig = parent.signature().unwrap();
        let head_commit = parent.head().unwrap().peel_to_commit().unwrap();
        parent
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "parent: add submodule gitlink",
                &tree,
                &[&head_commit],
            )
            .unwrap();

        dir
    }

    #[test]
    fn no_gitmodules_skips_silently() -> Result<()> {
        let dir = TempDir::new()?;
        Repository::init(dir.path())?;
        let scanner = SubmoduleIntegrityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty(), "expected no issues without .gitmodules");
        Ok(())
    }

    #[test]
    fn not_a_git_repo_skips_silently() -> Result<()> {
        let dir = TempDir::new()?;
        std::fs::write(
            dir.path().join(".gitmodules"),
            "[submodule \"x\"]\n\tpath = x\n\turl = x\n",
        )?;
        let scanner = SubmoduleIntegrityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        assert!(issues.is_empty());
        Ok(())
    }

    #[test]
    fn detects_tree_instead_of_gitlink() -> Result<()> {
        let dir = setup_parent_with_submodule();
        let dir_path = dir.path();

        // Corrupt: convert the submodule gitlink into a tree by removing the
        // gitlink and adding a file from inside the submodule directory as a
        // regular tracked file, then commit. This mirrors the infrahub
        // `git add levonk/` accident.
        let repo = Repository::open(dir_path)?;
        remove_gitlink(&repo, "levonk");
        leak_submodule_file(&repo, "levonk/README.md", b"# sub\n");

        // Commit the broken state, using the previous HEAD as parent.
        let mut idx = repo.index()?;
        let tree_oid = idx.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        let sig = repo.signature()?;
        let head_commit = repo.head()?.peel_to_commit()?;
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "broken: submodule as tree",
            &tree,
            &[&head_commit],
        )?;

        let scanner = SubmoduleIntegrityScanner::new();
        let issues = scanner.scan(&dir_path.to_string_lossy())?;
        assert!(
            !issues.is_empty(),
            "expected at least one issue for tree-instead-of-gitlink"
        );
        let any_error = issues
            .iter()
            .any(|i| i.severity == "error" && i.rule.contains("gitlink"));
        assert!(
            any_error,
            "expected an error about gitlink corruption: {:?}",
            issues
        );
        let has_fix = issues
            .iter()
            .any(|i| i.message.contains("git rm --cached -r"));
        assert!(has_fix, "expected fix command in message: {:?}", issues);
        Ok(())
    }

    #[test]
    fn clean_gitlink_has_no_errors() -> Result<()> {
        let dir = setup_parent_with_submodule();
        let scanner = SubmoduleIntegrityScanner::new();
        let issues = scanner.scan(&dir.path().to_string_lossy())?;
        let errors: Vec<_> = issues.iter().filter(|i| i.severity == "error").collect();
        assert!(
            errors.is_empty(),
            "expected no errors for a clean gitlink: {:?}",
            issues
        );
        Ok(())
    }

    #[test]
    fn index_check_flags_staged_leak() -> Result<()> {
        let dir = setup_parent_with_submodule();
        let dir_path = dir.path();
        let repo = Repository::open(dir_path)?;

        // Stage a leaked file under the submodule path without committing.
        remove_gitlink(&repo, "levonk");
        leak_submodule_file(&repo, "levonk/README.md", b"# sub\n");

        let scanner = SubmoduleIntegrityScanner::with_config(true);
        let issues = scanner.scan(&dir_path.to_string_lossy())?;
        // HEAD is still clean (gitlink), but the staged index check should fire.
        let staged_issue = issues
            .iter()
            .find(|i| i.rule == "submodule-gitlink-missing");
        assert!(
            staged_issue.is_some(),
            "expected a staged-index violation, got: {:?}",
            issues
        );
        Ok(())
    }
}
