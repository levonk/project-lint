# Security Rules

Security rules detect potential security vulnerabilities, hardcoded secrets, and insecure coding practices.

## Overview

Security rules help identify:
- Hardcoded secrets and credentials
- Insecure coding patterns
- Vulnerable dependencies
- Security misconfigurations
- Runtime security issues

## Configuration

Create `.config/project-lint/rules/active/security.toml`:

```toml
[[modular_rules]]
name = "security-scanner"
description = "Scans for security vulnerabilities and insecure practices"
enabled = true
severity = "error"
triggers = ["pre_write_code", "post_read_code"]

[modular_rules.security]
check_hardcoded_secrets = true
check_insecure_patterns = true
check_dependency_vulnerabilities = true
check_runtime_guards = true
```

## Security Categories

### 1. Hardcoded Secrets Detection

#### Patterns Detected
- API keys (AWS, Google, GitHub, etc.)
- Database connection strings
- Passwords and authentication tokens
- Private keys and certificates
- JWT tokens
- OAuth client secrets

#### Configuration
```toml
[[rules.custom_rules]]
name = "no-hardcoded-secrets"
pattern = "*"
triggers = ["pre_write_code"]
check_content = true
content_pattern = "(password|secret|token|key|api_key|auth).*=.*['\"][^'\"]{8,}['\"]"
severity = "error"
message = "Hardcoded secret detected. Use environment variables or secret management."
```

#### Examples
❌ **Bad:**
```javascript
const apiKey = "sk_live_1234567890abcdef";
const dbPassword = "mySecretPassword123";
```

✅ **Good:**
```javascript
const apiKey = process.env.API_KEY;
const dbPassword = process.env.DB_PASSWORD;
```

### 2. Insecure Coding Patterns

#### Common Issues
- SQL injection vulnerabilities
- XSS vulnerabilities
- Path traversal
- Command injection
- Insecure deserialization
- Weak cryptography

#### Configuration
```toml
[[rules.custom_rules]]
name = "prevent-sql-injection"
pattern = "*.[js|ts|py|php]"
check_content = true
content_pattern = "(query|execute).*\\+.*\\+"
severity = "error"
message = "Potential SQL injection. Use parameterized queries."
```

#### SQL Injection Prevention
❌ **Bad:**
```javascript
const query = "SELECT * FROM users WHERE id = " + userId;
db.query(query);
```

✅ **Good:**
```javascript
const query = "SELECT * FROM users WHERE id = ?";
db.query(query, [userId]);
```

#### XSS Prevention
❌ **Bad:**
```javascript
element.innerHTML = userInput;
```

✅ **Good:**
```javascript
element.textContent = userInput;
// or
element.innerHTML = sanitizeHtml(userInput);
```

### 3. Cryptographic Security

#### Secure Algorithms
- **Hashing**: SHA-256, SHA-384, SHA-512
- **Encryption**: AES-256, ChaCha20
- **Key Exchange**: ECDHE, DHE
- **Signatures**: ECDSA, EdDSA

#### Insecure Algorithms (Forbidden)
- **Hashing**: MD5, SHA-1, SHA-0
- **Encryption**: DES, 3DES, RC4, Blowfish
- **Key Exchange**: Static RSA, Anonymous DH

#### Configuration
```toml
[[rules.custom_rules]]
name = "no-insecure-crypto"
pattern = "*"
check_content = true
content_pattern = "(MD5|SHA1|DES|RC4|Blowfish)"
severity = "error"
message = "Insecure cryptographic algorithm detected. Use secure alternatives."
```

### 4. Dependency Security

#### Vulnerability Scanning
- Check for known CVEs
- Outdated dependencies
- Unmaintained packages
- License compliance

#### Configuration
```toml
[modular_rules.dependency_checker]
enabled = true
severity = "warning"
check_cve_database = true
max_days_outdated = 30
unmaintained_threshold = 180
```

### 5. Runtime Security Guards

#### Browser Security
- Detect browser-only APIs in server code
- Check for eval() usage
- Validate DOM manipulation patterns

#### Node.js Security
- Prevent require() of user input
- Check eval() and Function() usage
- Validate child_process usage

#### Configuration
```toml
[[rules.custom_rules]]
name = "runtime-guards"
pattern = "*.[js|ts]"
check_content = true
content_pattern = "eval\\(|Function\\(|require\\("
severity = "warning"
message = "Potentially unsafe runtime function detected."
```

## Language-Specific Security Rules

### JavaScript/TypeScript
```toml
[[modular_rules]]
name = "javascript-security"
description = "JavaScript/TypeScript security rules"
enabled = true
severity = "warning"

[[modular_rules.rules]]
name = "no-eval"
pattern = "*.[js|ts]"
check_content = true
content_pattern = "eval\\("
severity = "error"
message = "Avoid using eval() - potential code injection risk"

[[modular_rules.rules]]
name = "no-innerhtml"
pattern = "*.[js|ts]"
check_content = true
content_pattern = "\\.innerHTML\\s*="
severity = "warning"
message = "innerHTML can lead to XSS. Use textContent or sanitize HTML"
```

### Python
```toml
[[modular_rules]]
name = "python-security"
description = "Python security rules"
enabled = true
severity = "warning"

[[modular_rules.rules]]
name = "no-eval-python"
pattern = "*.py"
check_content = true
content_pattern = "eval\\(|exec\\("
severity = "error"
message = "Avoid using eval()/exec() in Python"

[[modular_rules.rules]]
name = "sql-injection-python"
pattern = "*.py"
check_content = true
content_pattern = "cursor\\.execute.*\\+.*\\+"
severity = "error"
message = "Potential SQL injection. Use parameterized queries"
```

### Rust
```toml
[[modular_rules]]
name = "rust-security"
description = "Rust security rules"
enabled = true
severity = "warning"

[[modular_rules.rules]]
name = "unsafe-blocks"
pattern = "*.rs"
check_content = true
content_pattern = "unsafe\\s*{"
severity = "warning"
message = "Unsafe block detected. Review for security implications"
```

## Security Best Practices

### 1. Secret Management
```toml
# Environment-based configuration
[security.secrets]
use_env_vars = true
required_env_vars = ["API_KEY", "DB_PASSWORD", "JWT_SECRET"]
forbidden_patterns = ["password", "secret", "token"]
```

### 2. Input Validation
```toml
[[rules.custom_rules]]
name = "input-validation"
pattern = "*.[js|ts|py]"
check_content = true
content_pattern = "(req\\.body|req\\.params|request\\.input).*\\.trim\\(\\)\\s*$"
severity = "warning"
message = "Input should be validated and sanitized"
```

### 3. Authentication & Authorization
```toml
[[rules.custom_rules]]
name = "auth-implementation"
pattern = "*.[js|ts]"
check_content = true
content_pattern = "==.*password"
severity = "error"
message = "Use secure password comparison (bcrypt, scrypt, argon2)"
```

## Integration with Security Tools

### 1. Snyk Integration
```toml
[security.snyk]
enabled = true
api_key_env = "SNYK_TOKEN"
severity_threshold = "medium"
```

### 2. OWASP Dependency Check
```toml
[security.owasp]
enabled = true
suppression_file = ".dependency-check-suppressions.xml"
fail_build_on_cvss = 7
```

### 3. Semgrep Integration
```toml
[security.semgrep]
enabled = true
rules_path = ".semgrep/rules"
config_file = ".semgrep.yml"
```

## Security Reporting

### Vulnerability Classification
- **Critical**: CVSS 9.0+ (Immediate action required)
- **High**: CVSS 7.0-8.9 (Fix within 7 days)
- **Medium**: CVSS 4.0-6.9 (Fix within 30 days)
- **Low**: CVSS 0.1-3.9 (Fix in next release)

### Security Metrics
```toml
[security.metrics]
track_vulnerability_count = true
track_fix_time = true
track_false_positives = true
generate_security_report = true
```

## Examples

### Security Rule Configuration
```toml
[[modular_rules]]
name = "comprehensive-security"
description = "Comprehensive security scanning"
enabled = true
severity = "error"
triggers = ["pre_write_code", "post_read_code"]

[[modular_rules.rules]]
name = "hardcoded-secrets"
pattern = "*"
check_content = true
content_pattern = "(password|secret|token|key|api_key).*=.*['\"][^'\"]{8,}['\"]"
severity = "error"

[[modular_rules.rules]]
name = "insecure-http"
pattern = "*"
check_content = true
content_pattern = "http://[^localhost]"
severity = "warning"

[[modular_rules.rules]]
name = "debug-code"
pattern = "*.[js|ts]"
check_content = true
content_pattern = "console\\.(log|debug|warn)"
severity = "info"
```

## Troubleshooting

### False Positives
1. Add legitimate patterns to exceptions
2. Use `.project-lint-security-ignore` file
3. Configure rule severity appropriately

### Performance Issues
1. Use specific file patterns
2. Exclude large dependencies
3. Cache security scan results

### Integration Issues
1. Verify API keys for external tools
2. Check network connectivity
3. Validate configuration files

## Security Checklist

### Development Phase
- [ ] No hardcoded secrets in code
- [ ] Input validation implemented
- [ ] Output encoding for XSS prevention
- [ ] Parameterized queries for database
- [ ] Secure password hashing
- [ ] HTTPS enforced in production

### Deployment Phase
- [ ] Environment variables configured
- [ ] Security headers set
- [ ] Dependency vulnerabilities scanned
- [ ] Error handling doesn't leak information
- [ ] Logging doesn't contain sensitive data
- [ ] Rate limiting implemented

### Monitoring Phase
- [ ] Security logging enabled
- [ ] Intrusion detection configured
- [ ] Regular security scans scheduled
- [ ] Vulnerability disclosure process
- [ ] Security incident response plan
