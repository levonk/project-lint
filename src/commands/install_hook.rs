use crate::utils::Result;
use clap::Args;
use std::path::PathBuf;
use std::fs;
use std::env;
use tracing::{info, warn, error};

#[derive(Args)]
pub struct InstallHookArgs {
    /// Target AI agent (windsurf, claude, cursor, generic)
    #[arg(long, default_value = "windsurf")]
    pub agent: String,

    /// Installation directory (defaults to agent's default location)
    #[arg(short, long)]
    pub dir: Option<String>,

    /// Force overwrite existing hooks
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: InstallHookArgs) -> Result<()> {
    info!("Installing project-lint hook for {} agent", args.agent);

    match args.agent.to_lowercase().as_str() {
        "windsurf" => install_windsurf_hook(&args).await?,
        "claude" => install_claude_hook(&args).await?,
        "cursor" => install_cursor_hook(&args).await?,
        "generic" => install_generic_hook(&args).await?,
        _ => {
            error!("Unsupported agent: {}", args.agent);
            return Err(anyhow::anyhow!("Unsupported agent: {}", args.agent));
        }
    }

    info!("Hook installation completed successfully");
    Ok(())
}

async fn install_windsurf_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".windsurf")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Windsurf hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

PROJECT_LINT_BIN="{}"
HOOK_TYPE="windsurf"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    // Create Windsurf configuration
    let config_content = r#"[hooks]
pre_tool_use = "./hook.sh"
post_tool_use = "./hook.sh"
pre_read_code = "./hook.sh"
post_read_code = "./hook.sh"
pre_write_code = "./hook.sh"
post_write_code = "./hook.sh"
"#;

    let config_path = hook_dir.join("config.toml");
    if !config_path.exists() || args.force {
        fs::write(&config_path, config_content)?;
        info!("Created Windsurf hook configuration at {:?}", config_path);
    }

    info!("Windsurf hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_claude_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".claude")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Claude Code hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

PROJECT_LINT_BIN="{}"
HOOK_TYPE="claude"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Claude Code hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_cursor_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, ".cursor")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Cursor hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

PROJECT_LINT_BIN="{}"
HOOK_TYPE="cursor"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
    );

    let hook_path = hook_dir.join("hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Cursor hook installed at {:?}", hook_path);
    Ok(())
}

async fn install_generic_hook(args: &InstallHookArgs) -> Result<()> {
    let hook_dir = get_hook_dir(&args.dir, "hooks")?;
    fs::create_dir_all(&hook_dir)?;

    let hook_content = format!(
        r#"#!/bin/bash
# Generic AI agent hook for project-lint
# This script intercepts tool execution and runs project-lint hooks

PROJECT_LINT_BIN="{}"
HOOK_TYPE="generic"

# Read the event from stdin
EVENT_DATA=$(cat)

# Pass the event to project-lint
echo "$EVENT_DATA" | "$PROJECT_LINT_BIN" hook --source "$HOOK_TYPE"
EXIT_CODE=$?

# Exit with the same code as project-lint
exit $EXIT_CODE
"#,
        env::current_exe()?.display()
    );

    let hook_path = hook_dir.join("project-lint-hook.sh");
    write_hook_file(&hook_path, &hook_content, args.force)?;
    make_executable(&hook_path)?;

    info!("Generic hook installed at {:?}", hook_path);
    Ok(())
}

fn get_hook_dir(custom_dir: &Option<String>, default_subdir: &str) -> Result<PathBuf> {
    if let Some(dir) = custom_dir {
        Ok(PathBuf::from(dir))
    } else {
        let cwd = env::current_dir()?;
        Ok(cwd.join(default_subdir))
    }
}

fn write_hook_file(path: &PathBuf, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        warn!("Hook file already exists at {:?}. Use --force to overwrite.", path);
        return Ok(());
    }

    fs::write(path, content)?;
    Ok(())
}

fn make_executable(path: &PathBuf) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    mod install_hook_tests;
}
