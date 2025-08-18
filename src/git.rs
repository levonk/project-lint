use git2::{BranchType, Repository};
use std::path::Path;
use tracing::{debug, warn};
use utils::Result;

pub struct GitInfo {
    pub current_branch: String,
    pub repository_path: std::path::PathBuf,
    pub is_clean: bool,
}

pub fn get_git_info(project_path: &str) -> Result<Option<GitInfo>> {
    let path = std::path::Path::new(project_path);

    match Repository::open(path) {
        Ok(repo) => {
            let head = repo.head()?;
            let current_branch = head
                .shorthand()
                .ok_or_else(|| anyhow::anyhow!("Could not get branch name"))?
                .to_string();

            let status = repo.statuses(None)?;
            let is_clean = status.is_empty();

            debug!("Git repository found at {:?}", repo.path());
            debug!("Current branch: {}", current_branch);
            debug!("Repository clean: {}", is_clean);

            Ok(Some(GitInfo {
                current_branch,
                repository_path: repo.path().parent().unwrap().to_path_buf(),
                is_clean,
            }))
        }
        Err(e) => {
            debug!("No git repository found at {:?}: {}", path, e);
            Ok(None)
        }
    }
}

pub fn check_branch_allowed(
    git_info: &GitInfo,
    allowed_branches: &[String],
    forbidden_branches: &[String],
) -> Result<bool> {
    let current_branch = &git_info.current_branch;

    // Check if branch is forbidden
    if forbidden_branches.contains(current_branch) {
        warn!("Current branch '{}' is in forbidden list", current_branch);
        return Ok(false);
    }

    // If allowed branches is empty, all branches are allowed (except forbidden ones)
    if allowed_branches.is_empty() {
        return Ok(true);
    }

    // Check if branch is explicitly allowed
    if allowed_branches.contains(current_branch) {
        return Ok(true);
    }

    warn!("Current branch '{}' is not in allowed list", current_branch);
    Ok(false)
}

pub fn get_branch_info(project_path: &str) -> Result<Option<String>> {
    if let Some(git_info) = get_git_info(project_path)? {
        Ok(Some(git_info.current_branch))
    } else {
        Ok(None)
    }
}

pub fn is_git_repository(path: &str) -> bool {
    let git_dir = std::path::Path::new(path).join(".git");
    git_dir.exists() && git_dir.is_dir()
}
