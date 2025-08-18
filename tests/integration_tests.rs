use project_lint::config::Config;
use std::path::Path;

#[test]
fn test_project_structure() {
    // Test that the project has the expected structure
    assert!(Path::new("src/main.rs").exists());
    assert!(Path::new("src/lib.rs").exists());
    assert!(Path::new("src/config.rs").exists());
    assert!(Path::new("src/git.rs").exists());
    assert!(Path::new("src/utils.rs").exists());
    assert!(Path::new("src/commands/mod.rs").exists());
    assert!(Path::new("src/commands/init.rs").exists());
    assert!(Path::new("src/commands/lint.rs").exists());
    assert!(Path::new("src/commands/watch.rs").exists());
    assert!(Path::new("Cargo.toml").exists());
    assert!(Path::new("README.md").exists());
    assert!(
        Path::new("docs-internal/requirements/20250804initial-project-lint-requirements.md")
            .exists()
    );
}

#[test]
fn test_config_defaults() {
    let config = Config::default();

    // Test git defaults
    assert!(config.git.warn_wrong_branch);
    assert!(config.git.allowed_branches.contains(&"main".to_string()));
    assert!(config.git.allowed_branches.contains(&"master".to_string()));
    assert!(config
        .git
        .forbidden_branches
        .contains(&"develop".to_string()));

    // Test files defaults
    assert!(config.files.auto_move);
    assert!(config.files.type_mappings.contains_key("*.sh"));
    assert_eq!(config.files.type_mappings["*.sh"], "bin/");

    // Test directories defaults
    assert!(config.directories.warn_scripts_location);
    assert_eq!(config.directories.scripts_directory, "bin");

    // Test rules defaults
    assert!(config
        .rules
        .enabled_checks
        .contains(&"git_branch".to_string()));
    assert!(config
        .rules
        .enabled_checks
        .contains(&"file_location".to_string()));
    assert!(config
        .rules
        .enabled_checks
        .contains(&"directory_structure".to_string()));
}

#[test]
fn test_pattern_matching() {
    // Test basic pattern matching functionality
    fn matches_pattern(file_name: &str, pattern: &str) -> bool {
        if pattern.starts_with('*') && pattern.ends_with('*') {
            file_name.contains(&pattern[1..pattern.len() - 1])
        } else if pattern.starts_with('*') {
            file_name.ends_with(&pattern[1..])
        } else if pattern.ends_with('*') {
            file_name.starts_with(&pattern[..pattern.len() - 1])
        } else {
            file_name == pattern
        }
    }

    assert!(matches_pattern("test.sh", "*.sh"));
    assert!(matches_pattern("script.py", "*.py"));
    assert!(matches_pattern("main.rs", "*.rs"));
    assert!(!matches_pattern("test.txt", "*.sh"));
    assert!(matches_pattern("test_file", "*file"));
    assert!(matches_pattern("file_test", "file*"));
}

#[test]
fn test_script_file_detection() {
    fn is_script_file(file_name: &str) -> bool {
        let script_extensions = [".sh", ".py", ".js", ".ts", ".rb", ".pl", ".php"];
        script_extensions.iter().any(|ext| file_name.ends_with(ext))
    }

    assert!(is_script_file("script.sh"));
    assert!(is_script_file("main.py"));
    assert!(is_script_file("app.js"));
    assert!(is_script_file("component.ts"));
    assert!(!is_script_file("document.txt"));
    assert!(!is_script_file("image.jpg"));
}
