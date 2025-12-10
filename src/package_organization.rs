/// Package organization rules for monorepo structure validation
/// Implements ADR 002: Refined Package Organization

use std::path::{Path, PathBuf};
use tracing::debug;

pub struct PackageOrganizationRuleSet;

impl PackageOrganizationRuleSet {
    /// Validate package path structure
    /// Expected: packages/{category}/{platform}/{domain}/{package-name}/{language}
    pub fn validate_package_path(path: &Path) -> Result<PackageValidation, String> {
        let path_str = path.to_string_lossy();
        let components: Vec<&str> = path_str.split('/').collect();

        // Find "packages" in the path
        let packages_idx = components
            .iter()
            .position(|&c| c == "packages")
            .ok_or_else(|| "Not a packages directory".to_string())?;

        let remaining = &components[packages_idx + 1..];

        if remaining.len() < 5 {
            return Err(format!(
                "Invalid package structure. Expected: packages/{{category}}/{{platform}}/{{domain}}/{{package-name}}/{{language}}, got: {}",
                remaining.join("/")
            ));
        }

        let category = remaining[0];
        let platform = remaining[1];
        let domain = remaining[2];
        let package_name = remaining[3];
        let language = remaining[4];

        // Validate category
        let valid_categories = ["core", "features", "services", "ui"];
        if !valid_categories.contains(&category) {
            return Err(format!(
                "Invalid category '{}'. Must be one of: {}",
                category,
                valid_categories.join(", ")
            ));
        }

        // Validate platform
        let valid_platforms = ["web", "node", "shared", "any"];
        if !valid_platforms.contains(&platform) {
            return Err(format!(
                "Invalid platform '{}'. Must be one of: {}",
                platform,
                valid_platforms.join(", ")
            ));
        }

        // Validate language
        let valid_languages = ["typescript", "python", "swift", "java", "go", "rust"];
        if !valid_languages.contains(&language) {
            return Err(format!(
                "Invalid language '{}'. Must be one of: {}",
                language,
                valid_languages.join(", ")
            ));
        }

        Ok(PackageValidation {
            category: category.to_string(),
            platform: platform.to_string(),
            domain: domain.to_string(),
            package_name: package_name.to_string(),
            language: language.to_string(),
            is_valid: true,
        })
    }

    /// Check for platform boundary violations
    /// Web packages should not import from node packages
    pub fn check_platform_boundaries(
        package_path: &Path,
        import_path: &str,
    ) -> Result<bool, String> {
        let validation = Self::validate_package_path(package_path)?;

        // If this is a web package, check if import is from node
        if validation.platform == "web" && import_path.contains("/node/") {
            return Ok(false); // Violation
        }

        // If this is a node package, check if import is from web
        if validation.platform == "node" && import_path.contains("/web/") {
            return Ok(false); // Violation
        }

        Ok(true) // No violation
    }
}

#[derive(Debug, Clone)]
pub struct PackageValidation {
    pub category: String,
    pub platform: String,
    pub domain: String,
    pub package_name: String,
    pub language: String,
    pub is_valid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_package_path() {
        let path = Path::new("packages/features/web/auth/auth-ui/typescript");
        let result = PackageOrganizationRuleSet::validate_package_path(path);
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert_eq!(validation.category, "features");
        assert_eq!(validation.platform, "web");
        assert_eq!(validation.domain, "auth");
        assert_eq!(validation.package_name, "auth-ui");
        assert_eq!(validation.language, "typescript");
    }

    #[test]
    fn test_invalid_category() {
        let path = Path::new("packages/invalid/web/auth/auth-ui/typescript");
        let result = PackageOrganizationRuleSet::validate_package_path(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_platform() {
        let path = Path::new("packages/features/invalid/auth/auth-ui/typescript");
        let result = PackageOrganizationRuleSet::validate_package_path(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_language() {
        let path = Path::new("packages/features/web/auth/auth-ui/invalid");
        let result = PackageOrganizationRuleSet::validate_package_path(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_platform_boundary_violation() {
        let package_path = Path::new("packages/features/web/auth/auth-ui/typescript");
        let result = PackageOrganizationRuleSet::check_platform_boundaries(
            package_path,
            "packages/features/node/auth/core/typescript",
        );
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should be a violation
    }

    #[test]
    fn test_platform_boundary_valid() {
        let package_path = Path::new("packages/features/web/auth/auth-ui/typescript");
        let result = PackageOrganizationRuleSet::check_platform_boundaries(
            package_path,
            "packages/features/shared/auth/types/typescript",
        );
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should be valid
    }
}
