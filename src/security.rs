/// Security-specific detection rules using the generic detection module
/// Implements codeguard rules for static analysis

use crate::detection::{DetectionIssue, FunctionCallDetector, FunctionCallRule, PatternDetector, PatternRule};
use std::path::Path;
use tracing::info;

pub struct SecurityRuleSet;

impl SecurityRuleSet {
    /// Get hardcoded credentials detection rules
    pub fn hardcoded_credentials_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "aws_key".to_string(),
                pattern: r"(AKIA|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ASIA)[0-9A-Z]{16}".to_string(),
                severity: "critical".to_string(),
                message_template: "🔐 AWS access key detected: {matched}. This is a hardcoded credential and must be removed immediately.".to_string(),
                fix_template: Some("os.getenv('AWS_ACCESS_KEY_ID')".to_string()),
                case_sensitive: true,
            },
            PatternRule {
                name: "stripe_key".to_string(),
                pattern: r"(sk_live_|pk_live_|sk_test_|pk_test_)[A-Za-z0-9]{20,}".to_string(),
                severity: "critical".to_string(),
                message_template: "🔐 Stripe API key detected: {matched}. This is a hardcoded credential and must be removed immediately.".to_string(),
                fix_template: Some("os.getenv('STRIPE_KEY')".to_string()),
                case_sensitive: true,
            },
            PatternRule {
                name: "github_token".to_string(),
                pattern: r"(ghp_|gho_|ghu_|ghs_|ghr_)[A-Za-z0-9_]{36,255}".to_string(),
                severity: "critical".to_string(),
                message_template: "🔐 GitHub token detected: {matched}. This is a hardcoded credential and must be removed immediately.".to_string(),
                fix_template: Some("os.getenv('GITHUB_TOKEN')".to_string()),
                case_sensitive: true,
            },
            PatternRule {
                name: "jwt_token".to_string(),
                pattern: r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+".to_string(),
                severity: "critical".to_string(),
                message_template: "🔐 JWT token detected: {matched}. This is a hardcoded credential and must be removed immediately.".to_string(),
                fix_template: Some("os.getenv('JWT_TOKEN')".to_string()),
                case_sensitive: true,
            },
            PatternRule {
                name: "private_key".to_string(),
                pattern: r"-----BEGIN.*PRIVATE KEY-----".to_string(),
                severity: "critical".to_string(),
                message_template: "🔐 Private key block detected. This is a hardcoded credential and must be removed immediately.".to_string(),
                fix_template: Some("# Load from secure key management system".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "connection_string_with_creds".to_string(),
                pattern: r"(mongodb|mysql|postgres|mssql)://[^:]+:[^@]+@".to_string(),
                severity: "critical".to_string(),
                message_template: "🔐 Connection string with credentials detected: {matched}. Move credentials to environment variables.".to_string(),
                fix_template: Some("os.getenv('DATABASE_URL')".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "suspicious_password_var".to_string(),
                pattern: r"(password|secret|api_key|token|auth)\s*=\s*['\"][^'\"]{8,}['\"]".to_string(),
                severity: "high".to_string(),
                message_template: "⚠️  Suspicious variable assignment detected: {matched}. Review and move to environment variables.".to_string(),
                fix_template: Some("os.getenv('CREDENTIAL_NAME')".to_string()),
                case_sensitive: false,
            },
        ]
    }

    /// Get insecure C function detection rules
    pub fn insecure_c_functions_rules() -> Vec<FunctionCallRule> {
        vec![
            FunctionCallRule {
                name: "unsafe_gets".to_string(),
                function_names: vec!["gets".to_string()],
                severity: "critical".to_string(),
                message_template: "❌ Unsafe function '{function}()' detected at {file}:{line}. Replace with fgets() for bounds checking.".to_string(),
                fix_template: Some("fgets".to_string()),
            },
            FunctionCallRule {
                name: "unsafe_strcpy".to_string(),
                function_names: vec!["strcpy".to_string()],
                severity: "high".to_string(),
                message_template: "❌ Unsafe function '{function}()' detected at {file}:{line}. Replace with strcpy_s() or snprintf() for bounds checking.".to_string(),
                fix_template: Some("snprintf".to_string()),
            },
            FunctionCallRule {
                name: "unsafe_strcat".to_string(),
                function_names: vec!["strcat".to_string()],
                severity: "high".to_string(),
                message_template: "❌ Unsafe function '{function}()' detected at {file}:{line}. Replace with strcat_s() or snprintf() for bounds checking.".to_string(),
                fix_template: Some("snprintf".to_string()),
            },
            FunctionCallRule {
                name: "unsafe_sprintf".to_string(),
                function_names: vec!["sprintf".to_string()],
                severity: "high".to_string(),
                message_template: "❌ Unsafe function '{function}()' detected at {file}:{line}. Replace with snprintf() for bounds checking.".to_string(),
                fix_template: Some("snprintf".to_string()),
            },
            FunctionCallRule {
                name: "unsafe_scanf".to_string(),
                function_names: vec!["scanf".to_string()],
                severity: "high".to_string(),
                message_template: "❌ Unsafe function '{function}()' detected at {file}:{line}. Use fgets() + sscanf() or add width specifiers.".to_string(),
                fix_template: Some("fgets".to_string()),
            },
            FunctionCallRule {
                name: "unsafe_strtok".to_string(),
                function_names: vec!["strtok".to_string()],
                severity: "high".to_string(),
                message_template: "❌ Unsafe function '{function}()' detected at {file}:{line}. Replace with strtok_s() or strtok_r() for thread safety.".to_string(),
                fix_template: Some("strtok_r".to_string()),
            },
            FunctionCallRule {
                name: "unsafe_memcpy".to_string(),
                function_names: vec!["memcpy".to_string()],
                severity: "medium".to_string(),
                message_template: "⚠️  Function '{function}()' detected at {file}:{line}. Consider using memcpy_s() for bounds checking.".to_string(),
                fix_template: Some("memcpy_s".to_string()),
            },
        ]
    }

    /// Get insecure crypto detection rules
    pub fn insecure_crypto_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "md5_usage".to_string(),
                pattern: r"\b(MD5|md5)\b".to_string(),
                severity: "high".to_string(),
                message_template: "🚫 MD5 hash algorithm detected: {matched}. MD5 is cryptographically broken. Use SHA-256 or stronger.".to_string(),
                fix_template: Some("SHA256".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "sha1_usage".to_string(),
                pattern: r"\b(SHA-?1|sha-?1)\b".to_string(),
                severity: "high".to_string(),
                message_template: "🚫 SHA-1 algorithm detected: {matched}. SHA-1 is deprecated. Use SHA-256 or stronger.".to_string(),
                fix_template: Some("SHA256".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "des_usage".to_string(),
                pattern: r"\b(DES|des)\b(?!k)".to_string(),
                severity: "high".to_string(),
                message_template: "🚫 DES encryption detected: {matched}. DES is insecure. Use AES-256 or stronger.".to_string(),
                fix_template: Some("AES".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "rc4_usage".to_string(),
                pattern: r"\b(RC4|rc4)\b".to_string(),
                severity: "high".to_string(),
                message_template: "🚫 RC4 cipher detected: {matched}. RC4 is broken. Use AES-GCM or ChaCha20.".to_string(),
                fix_template: Some("ChaCha20".to_string()),
                case_sensitive: false,
            },
            PatternRule {
                name: "blowfish_usage".to_string(),
                pattern: r"\b(Blowfish|blowfish)\b".to_string(),
                severity: "medium".to_string(),
                message_template: "⚠️  Blowfish cipher detected: {matched}. Consider using AES-256 for better security.".to_string(),
                fix_template: Some("AES".to_string()),
                case_sensitive: false,
            },
        ]
    }

    /// Get certificate detection rules
    pub fn certificate_rules() -> Vec<PatternRule> {
        vec![
            PatternRule {
                name: "pem_certificate".to_string(),
                pattern: r"-----BEGIN CERTIFICATE-----".to_string(),
                severity: "info".to_string(),
                message_template: "ℹ️  PEM certificate block detected. Verify certificate validity and key strength.".to_string(),
                fix_template: None,
                case_sensitive: true,
            },
            PatternRule {
                name: "self_signed_cert".to_string(),
                pattern: r"self.?signed|self_signed".to_string(),
                severity: "info".to_string(),
                message_template: "ℹ️  Self-signed certificate detected. Ensure this is intentional and only used for development/testing.".to_string(),
                fix_template: None,
                case_sensitive: false,
            },
        ]
    }
}

pub struct SecurityScanner {
    credentials_detector: PatternDetector,
    c_functions_detector: FunctionCallDetector,
    crypto_detector: PatternDetector,
    certificate_detector: PatternDetector,
}

impl SecurityScanner {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            credentials_detector: PatternDetector::new(
                SecurityRuleSet::hardcoded_credentials_rules(),
            )?,
            c_functions_detector: FunctionCallDetector::new(
                SecurityRuleSet::insecure_c_functions_rules(),
            ),
            crypto_detector: PatternDetector::new(SecurityRuleSet::insecure_crypto_rules())?,
            certificate_detector: PatternDetector::new(SecurityRuleSet::certificate_rules())?,
        })
    }

    /// Scan a file for all security issues
    pub fn scan_file(&self, file_path: &Path) -> Result<Vec<DetectionIssue>, Box<dyn std::error::Error>> {
        let mut all_issues = Vec::new();

        // Determine file type to decide which detectors to run
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let is_c_file = file_name.ends_with(".c") || file_name.ends_with(".h");
        let is_code_file = file_name.ends_with(".rs")
            || file_name.ends_with(".py")
            || file_name.ends_with(".js")
            || file_name.ends_with(".ts")
            || file_name.ends_with(".go")
            || is_c_file;

        // Always scan for credentials
        all_issues.extend(self.credentials_detector.scan_file(file_path)?);

        // Scan for insecure crypto
        all_issues.extend(self.crypto_detector.scan_file(file_path)?);

        // Scan for certificates
        all_issues.extend(self.certificate_detector.scan_file(file_path)?);

        // Only scan C files for unsafe functions
        if is_c_file {
            all_issues.extend(self.c_functions_detector.scan_file(file_path)?);
        }

        Ok(all_issues)
    }

    /// Apply fixes to a file
    pub fn apply_fixes(
        &self,
        file_path: &Path,
        issues: &[DetectionIssue],
        dry_run: bool,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let (_, fixes_applied) = self.credentials_detector.apply_fixes(file_path, issues, dry_run)?;
        Ok(fixes_applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_rules_created() {
        let cred_rules = SecurityRuleSet::hardcoded_credentials_rules();
        assert!(!cred_rules.is_empty());

        let c_rules = SecurityRuleSet::insecure_c_functions_rules();
        assert!(!c_rules.is_empty());

        let crypto_rules = SecurityRuleSet::insecure_crypto_rules();
        assert!(!crypto_rules.is_empty());

        let cert_rules = SecurityRuleSet::certificate_rules();
        assert!(!cert_rules.is_empty());
    }

    #[test]
    fn test_scanner_creation() {
        let scanner = SecurityScanner::new();
        assert!(scanner.is_ok());
    }
}
