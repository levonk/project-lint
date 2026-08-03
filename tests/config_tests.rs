use project_lint_core::config::Config;

#[test]
fn test_default_config() {
    let config = Config::default();

    assert!(config.git.warn_wrong_branch);
    assert!(config.files.auto_move);
    assert!(config.directories.warn_scripts_location);
    assert_eq!(config.directories.scripts_directory, "bin");
}

#[test]
fn test_config_serialization() {
    let config = Config::default();
    let toml_string = toml::to_string_pretty(&config).unwrap();

    // Should be able to deserialize back
    let parsed_config: Config = toml::from_str(&toml_string).unwrap();

    assert_eq!(
        config.git.warn_wrong_branch,
        parsed_config.git.warn_wrong_branch
    );
    assert_eq!(config.files.auto_move, parsed_config.files.auto_move);
    assert_eq!(
        config.directories.scripts_directory,
        parsed_config.directories.scripts_directory
    );
}

#[test]
fn test_custom_rules() {
    let mut config = Config::default();

    config
        .rules
        .custom_rules
        .push(project_lint_core::config::CustomRule {
            name: "test_rule".to_string(),
            pattern: "*.test".to_string(),
            message: "Test rule".to_string(),
            severity: project_lint_core::config::RuleSeverity::Warning,
            check_content: false,
            content_pattern: None,
            exception_pattern: None,
            condition: None,
            required: false,
            required_if_path_exists: None,
            triggers: vec![],
            mode: project_lint_core::config::ExecutionMode::LocalSync,
        });

    assert_eq!(config.rules.custom_rules.len(), 1);
    assert_eq!(config.rules.custom_rules[0].name, "test_rule");
}
