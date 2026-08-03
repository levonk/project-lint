use crate::file_naming::FileNamingRule;
use crate::utils::Result;
use tempfile::TempDir;
use std::fs;
use std::path::Path;

#[test]
fn test_file_naming_validation() -> Result<()> {
    let rule = FileNamingRule::new();
    
    // Test valid names
    assert!(rule.is_valid_name("my-component.ts"));
    assert!(rule.is_valid_name("utils.ts"));
    assert!(rule.is_valid_name("README.md"));
    
    // Test invalid names
    assert!(!rule.is_valid_name("MyComponent.ts"));
    assert!(!rule.is_valid_name("temp file.txt"));
    assert!(!rule.is_valid_name("file.js~"));
    
    Ok(())
}

#[test]
fn test_file_extension_validation() -> Result<()> {
    let rule = FileNamingRule::new();
    
    // Test valid extensions
    assert!(rule.has_valid_extension("component.ts"));
    assert!(rule.has_valid_extension("script.js"));
    assert!(rule.has_valid_extension("module.py"));
    
    // Test invalid extensions
    assert!(!rule.has_valid_extension("file.tmp"));
    assert!(!rule.has_valid_extension("backup.bak"));
    
    Ok(())
}

#[test]
fn test_temporary_file_detection() -> Result<()> {
    let rule = FileNamingRule::new();
    
    // Should detect temp files
    assert!(rule.is_temporary_file("test.tmp"));
    assert!(rule.is_temporary_file("backup.bak"));
    assert!(rule.is_temporary_file("old~"));
    
    // Should not detect regular files
    assert!(!rule.is_temporary_file("component.ts"));
    assert!(!rule.is_temporary_file("README.md"));
    
    Ok(())
}

#[test]
fn test_case_sensitivity_rules() -> Result<()> {
    let rule = FileNamingRule::new();
    
    // Test kebab-case (preferred)
    assert!(rule.is_preferred_case("my-component.ts"));
    assert!(rule.is_preferred_case("user-service.js"));
    
    // Test camelCase (acceptable in JS)
    assert!(rule.is_acceptable_case("myComponent.ts", "typescript"));
    assert!(!rule.is_acceptable_case("MyComponent.ts", "typescript"));
    
    Ok(())
}

#[test]
fn test_standard_file_names() -> Result<()> {
    let rule = FileNamingRule::new();
    
    // Test standard files
    assert!(rule.is_standard_file("README.md"));
    assert!(rule.is_standard_file("LICENSE"));
    assert!(rule.is_standard_file("package.json"));
    assert!(rule.is_standard_file("Cargo.toml"));
    
    // Test non-standard files
    assert!(!rule.is_standard_file("readme.txt"));
    assert!(!rule.is_standard_file("Package.json"));
    
    Ok(())
}

#[test]
fn test_file_organization_rules() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let project_root = temp_dir.path();
    
    // Create test structure
    fs::create_dir_all(project_root.join("src/components"))?;
    fs::create_dir_all(project_root.join("src/utils"))?;
    fs::create_dir_all(project_root.join("tests"))?;
    
    let rule = FileNamingRule::new();
    
    // Test correct organization
    assert!(rule.is_well_organized(
        project_root.join("src/components/button.ts")
    )?);
    
    // Test misplaced files
    assert!(!rule.is_well_organized(
        project_root.join("src/button.ts")
    )?);
    
    Ok(())
}
