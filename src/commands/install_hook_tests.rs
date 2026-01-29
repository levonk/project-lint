use crate::commands::install_hook::{run, InstallHookArgs};
use crate::utils::Result;
use tempfile::TempDir;
use std::fs;
use std::path::Path;
use tokio::fs as async_fs;

#[tokio::test]
async fn test_install_windsurf_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let hook_dir = temp_dir.path().join(".windsurf");
    
    let args = InstallHookArgs {
        agent: "windsurf".to_string(),
        dir: Some(hook_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check hook script was created
    let hook_script = hook_dir.join("hook.sh");
    assert!(hook_script.exists());
    
    // Check script is executable
    let metadata = fs::metadata(&hook_script)?;
    #[cfg(unix)]
    assert!(metadata.permissions().mode() & 0o111 != 0);
    
    // Check config file was created
    let config_file = hook_dir.join("config.toml");
    assert!(config_file.exists());
    
    // Verify script content
    let content = fs::read_to_string(&hook_script)?;
    assert!(content.contains("project-lint hook --source windsurf"));
    
    Ok(())
}

#[tokio::test]
async fn test_install_claude_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let hook_dir = temp_dir.path().join(".claude");
    
    let args = InstallHookArgs {
        agent: "claude".to_string(),
        dir: Some(hook_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check hook script was created
    let hook_script = hook_dir.join("hook.sh");
    assert!(hook_script.exists());
    
    // Verify script content
    let content = fs::read_to_string(&hook_script)?;
    assert!(content.contains("project-lint hook --source claude"));
    
    Ok(())
}

#[tokio::test]
async fn test_install_cursor_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let hook_dir = temp_dir.path().join(".cursor");
    
    let args = InstallHookArgs {
        agent: "cursor".to_string(),
        dir: Some(hook_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check hook script was created
    let hook_script = hook_dir.join("hook.sh");
    assert!(hook_script.exists());
    
    // Verify script content
    let content = fs::read_to_string(&hook_script)?;
    assert!(content.contains("project-lint hook --source cursor"));
    
    Ok(())
}

#[tokio::test]
async fn test_install_generic_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let hook_dir = temp_dir.path().join("hooks");
    
    let args = InstallHookArgs {
        agent: "generic".to_string(),
        dir: Some(hook_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    run(args).await?;
    
    // Check hook script was created
    let hook_script = hook_dir.join("project-lint-hook.sh");
    assert!(hook_script.exists());
    
    // Verify script content
    let content = fs::read_to_string(&hook_script)?;
    assert!(content.contains("project-lint hook --source generic"));
    
    Ok(())
}

#[tokio::test]
async fn test_install_hook_force_overwrite() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let hook_dir = temp_dir.path().join(".windsurf");
    fs::create_dir_all(&hook_dir)?;
    
    // Create existing hook
    let existing_hook = hook_dir.join("hook.sh");
    fs::write(&existing_hook, "existing content")?;
    
    let args = InstallHookArgs {
        agent: "windsurf".to_string(),
        dir: Some(hook_dir.to_string_lossy().to_string()),
        force: false,
    };
    
    // Should not overwrite without force
    run(args).await?;
    let content = fs::read_to_string(&existing_hook)?;
    assert_eq!(content, "existing content");
    
    // Now with force
    let args = InstallHookArgs {
        agent: "windsurf".to_string(),
        dir: Some(hook_dir.to_string_lossy().to_string()),
        force: true,
    };
    
    run(args).await?;
    let content = fs::read_to_string(&existing_hook)?;
    assert!(content.contains("project-lint hook --source windsurf"));
    
    Ok(())
}

#[tokio::test]
async fn test_install_hook_unsupported_agent() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let args = InstallHookArgs {
        agent: "unsupported".to_string(),
        dir: Some(temp_dir.path().to_string_lossy().to_string()),
        force: false,
    };
    
    let result = run(args).await;
    assert!(result.is_err());
    
    Ok(())
}
