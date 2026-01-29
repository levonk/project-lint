use crate::config::Config;
use crate::utils::Result;
use tempfile::TempDir;
use std::fs;
use std::path::Path;

#[test]
fn test_config_load_default() -> Result<()> {
    let config = Config::default();
    
    // Test default values
    assert!(config.git.enabled);
    assert!(config.files.enabled);
    assert!(config.directories.enabled);
    
    Ok(())
}

#[test]
fn test_config_load_from_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("config.toml");
    
    let config_content = r#"
[git]
enabled = false
default_branch = "main"

[files]
enabled = true
case_sensitive = true

[[rules.custom_rules]]
name = "test-rule"
pattern = "*.test"
severity = "warning"
message = "Test rule"
"#;
    
    fs::write(&config_file, config_content)?;
    
    let config = Config::load_from_path(&config_file)?;
    
    assert!(!config.git.enabled);
    assert_eq!(config.git.default_branch, "main");
    assert!(config.files.enabled);
    assert!(config.files.case_sensitive);
    assert_eq!(config.rules.custom_rules.len(), 1);
    assert_eq!(config.rules.custom_rules[0].name, "test-rule");
    
    Ok(())
}

#[test]
fn test_config_merge() -> Result<()> {
    let base_config = Config {
        git: crate::config::GitConfig {
            enabled: true,
            default_branch: "main".to_string(),
            ..Default::default()
        },
        files: crate::config::FilesConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    
    let override_config = Config {
        git: crate::config::GitConfig {
            enabled: false,
            default_branch: "develop".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    
    let merged = Config::merge(base_config, override_config)?;
    
    assert!(!merged.git.enabled);
    assert_eq!(merged.git.default_branch, "develop");
    assert!(!merged.files.enabled);
    
    Ok(())
}

#[test]
fn test_config_validation() -> Result<()> {
    let mut config = Config::default();
    
    // Valid config should pass
    assert!(config.validate().is_ok());
    
    // Invalid severity should fail
    if let Some(rule) = config.rules.custom_rules.get_mut(0) {
        rule.severity = crate::config::RuleSeverity::Error;
    }
    
    // This should still be valid as Error is a valid severity
    assert!(config.validate().is_ok());
    
    Ok(())
}

#[test]
fn test_modular_rules() -> Result<()> {
    let mut config = Config::default();
    
    let modular_rule = crate::config::ModularRule {
        name: "test-modular".to_string(),
        description: "Test modular rule".to_string(),
        enabled: true,
        severity: crate::config::RuleSeverity::Warning,
        triggers: vec!["pre_write_code".to_string()],
        ..Default::default()
    };
    
    config.modular_rules.push(modular_rule);
    
    assert_eq!(config.modular_rules.len(), 1);
    assert!(config.modular_rules[0].enabled);
    assert_eq!(config.modular_rules[0].name, "test-modular");
    
    Ok(())
}

#[test]
fn test_custom_rule_patterns() -> Result<()> {
    let rule = crate::config::CustomRule {
        name: "pattern-test".to_string(),
        pattern: "*.rs".to_string(),
        message: "Test pattern".to_string(),
        severity: crate::config::RuleSeverity::Info,
        check_content: true,
        content_pattern: Some("unsafe".to_string()),
        condition: Some("contains".to_string()),
        required: false,
        triggers: vec!["post_read_code".to_string()],
        ..Default::default()
    };
    
    assert_eq!(rule.pattern, "*.rs");
    assert!(rule.check_content);
    assert_eq!(rule.content_pattern, Some("unsafe".to_string()));
    assert_eq!(rule.condition, Some("contains".to_string()));
    
    Ok(())
}

#[test]
fn test_rule_triggers() -> Result<()> {
    let valid_triggers = vec![
        "pre_tool_use",
        "post_tool_use", 
        "pre_read_code",
        "post_read_code",
        "pre_write_code",
        "post_write_code",
        "pre_run_command",
        "post_run_command",
        "pre_user_prompt",
        "post_model_response",
    ];
    
    for trigger in valid_triggers {
        assert!(crate::config::is_valid_trigger(trigger));
    }
    
    assert!(!crate::config::is_valid_trigger("invalid_trigger"));
    
    Ok(())
}

#[test]
fn test_config_file_paths() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    // Test user config path
    let user_config = dirs::home_dir()
        .unwrap_or_else(|| temp_dir.path().to_path_buf())
        .join(".config")
        .join("project-lint")
        .join("config.toml");
    
    // Test project config path
    let project_config = temp_dir.path().join(".config").join("project-lint").join("config.toml");
    
    // Test local config path
    let local_config = temp_dir.path().join("project-lint.toml");
    
    // Create local config
    fs::create_dir_all(local_config.parent().unwrap())?;
    fs::write(&local_config, r#"[git]
enabled = true
"#)?;
    
    let config = Config::load_auto(temp_dir.path())?;
    assert!(config.git.enabled);
    
    Ok(())
}

#[test]
fn test_rule_severity_conversion() -> Result<()> {
    let severities = vec![
        ("error", crate::config::RuleSeverity::Error),
        ("warning", crate::config::RuleSeverity::Warning),
        ("info", crate::config::RuleSeverity::Info),
    ];
    
    for (str, expected) in severities {
        let parsed = crate::config::parse_severity(str)?;
        assert_eq!(parsed, expected);
    }
    
    // Test invalid severity
    assert!(crate::config::parse_severity("invalid").is_err());
    
    Ok(())
}

#[test]
fn test_config_serialization() -> Result<()> {
    let config = Config::default();
    
    // Test serialization
    let serialized = toml::to_string_pretty(&config)?;
    assert!(!serialized.is_empty());
    
    // Test deserialization
    let deserialized: Config = toml::from_str(&serialized)?;
    assert_eq!(config.git.enabled, deserialized.git.enabled);
    assert_eq!(config.files.enabled, deserialized.files.enabled);
    
    Ok(())
}

#[test]
fn test_profile_configuration() -> Result<()> {
    let mut config = Config::default();
    
    let profile = crate::config::Profile {
        metadata: crate::config::ProfileMetadata {
            name: "test-profile".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        },
        activation: crate::config::ProfileActivation {
            auto: true,
            conditions: vec![],
            ..Default::default()
        },
        enable: crate::config::ProfileEnable {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    
    config.active_profiles.push(profile);
    
    assert_eq!(config.active_profiles.len(), 1);
    assert_eq!(config.active_profiles[0].metadata.name, "test-profile");
    assert!(config.active_profiles[0].enable.enabled);
    
    Ok(())
}
