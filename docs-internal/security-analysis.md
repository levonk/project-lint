# Security Analysis Module

## Overview

The security analysis module adds static security scanning capabilities to project-lint, implementing codeguard rules for detecting common security vulnerabilities without executing code.

## Architecture

### Generic Detection Module (`src/detection.rs`)

The detection module provides reusable, language-agnostic pattern and function call detection:

- **`PatternDetector`**: Regex-based pattern matching for detecting strings, configurations, and code patterns
- **`FunctionCallDetector`**: Detects function calls by name with regex-based matching
- **`DetectionIssue`**: Unified issue representation with severity, message, and optional fix

**Key Features:**
- Case-sensitive and case-insensitive matching
- Message templating with variable substitution
- Automatic fix suggestions
- Dry-run mode for previewing fixes

### Security Module (`src/security.rs`)

The security module implements specific security rules using the generic detection framework:

- **`SecurityRuleSet`**: Defines all security rules organized by category
- **`SecurityScanner`**: Orchestrates scanning across multiple rule categories
- **`SecurityIssueType`**: Categorizes issues (credentials, C functions, crypto, certificates)

## Security Rules

### 1. Hardcoded Credentials Detection

Detects common secret patterns in source code:

- **AWS Keys**: Prefixes `AKIA`, `AGPA`, `AIDA`, `AROA`, `AIPA`, `ANPA`, `ANVA`, `ASIA`
- **Stripe Keys**: Prefixes `sk_live_`, `pk_live_`, `sk_test_`, `pk_test_`
- **GitHub Tokens**: Prefixes `ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`
- **JWT Tokens**: Pattern `eyJ[...].eyJ[...].eyJ[...]`
- **Private Keys**: Blocks between `-----BEGIN` and `-----END PRIVATE KEY-----`
- **Connection Strings**: URLs with embedded credentials (MongoDB, MySQL, PostgreSQL, MSSQL)
- **Suspicious Variables**: Variables named `password`, `secret`, `api_key`, `token`, `auth` with string values

**Severity**: Critical

**Auto-fix**: Replaces with environment variable references (e.g., `os.getenv('AWS_ACCESS_KEY_ID')`)

### 2. Insecure C Functions

Detects unsafe C/C++ functions that can cause buffer overflows:

- **`gets()`**: No bounds checking (CRITICAL)
- **`strcpy()`**: No bounds checking (HIGH)
- **`strcat()`**: No bounds checking (HIGH)
- **`sprintf()`**: No bounds checking (HIGH)
- **`scanf()`**: Unbounded input without width specifiers (HIGH)
- **`strtok()`**: Not thread-safe (HIGH)
- **`memcpy()`**: Requires careful size validation (MEDIUM)

**Severity**: Critical to Medium

**Auto-fix**: Suggests safe alternatives:
- `gets()` → `fgets()`
- `strcpy()` → `snprintf()` or `strcpy_s()`
- `strcat()` → `snprintf()` or `strcat_s()`
- `sprintf()` → `snprintf()`
- `scanf()` → `fgets()` + `sscanf()`
- `strtok()` → `strtok_r()` or `strtok_s()`
- `memcpy()` → `memcpy_s()`

### 3. Insecure Cryptography

Detects use of broken or deprecated cryptographic algorithms:

- **MD5**: Cryptographically broken
- **SHA-1**: Deprecated
- **DES**: Insecure
- **RC4**: Broken
- **Blowfish**: Weak

**Severity**: High

**Auto-fix**: Suggests secure alternatives:
- MD5 → SHA-256
- SHA-1 → SHA-256
- DES → AES
- RC4 → ChaCha20
- Blowfish → AES

### 4. Certificate Issues

Detects certificate files and embedded certificates for manual review:

- **PEM Certificates**: `-----BEGIN CERTIFICATE-----` blocks
- **Self-Signed Certificates**: Marked as self-signed

**Severity**: Informational

**Auto-fix**: None (requires manual review)

## Usage

### Basic Scanning

```bash
# Run security analysis
project-lint lint

# Security issues will be reported with emoji indicators:
# 🔐 Critical credentials issues
# ❌ Unsafe C functions
# 🚫 Insecure crypto
# ℹ️  Certificate information
```

### Dry-Run Mode

Preview what would be fixed without making changes:

```bash
project-lint lint --dry-run
```

Output:
```
📋 Would apply 5 security fixes
```

### Auto-Fix Mode

Automatically apply fixes to detected issues:

```bash
project-lint lint --fix
```

Output:
```
✅ Applied 5 security fixes
Fixed 2 issues in src/auth.rs
Fixed 3 issues in src/crypto.rs
```

## File Type Support

Security scanning runs on source files:

- **Python**: `.py`
- **Rust**: `.rs`
- **JavaScript/TypeScript**: `.js`, `.ts`, `.tsx`, `.jsx`
- **Go**: `.go`
- **C/C++**: `.c`, `.h`, `.cpp`
- **Java**: `.java`
- **C#**: `.cs`

### Unsafe C Function Detection

Only runs on C/C++ files (`.c`, `.h`, `.cpp`)

### Other Detections

Run on all supported source files

## Integration with Lint Command

The security analysis is automatically integrated into the lint command:

1. Loads configuration
2. Determines active profiles
3. Processes modular rules
4. Performs AST analysis
5. **Performs security analysis** ← New
6. Performs legacy checks
7. Reports all issues

## Performance Considerations

- **Skipped Directories**: `node_modules`, `target`, `.git`
- **Skipped Files**: Minified files (`.min.js`), lock files, hidden files
- **Regex Compilation**: Patterns are compiled once at scanner initialization
- **Parallel Potential**: File scanning can be parallelized in future versions

## Extending Security Rules

To add new security rules:

1. Add a new method to `SecurityRuleSet` returning `Vec<PatternRule>` or `Vec<FunctionCallRule>`
2. Create a detector instance in `SecurityScanner::new()`
3. Call the detector in `SecurityScanner::scan_file()`

Example:

```rust
pub fn new_security_rules() -> Vec<PatternRule> {
    vec![
        PatternRule {
            name: "my_rule".to_string(),
            pattern: r"pattern_here".to_string(),
            severity: "high".to_string(),
            message_template: "Found issue: {matched}".to_string(),
            fix_template: Some("replacement".to_string()),
            case_sensitive: false,
        },
    ]
}
```

## Configuration

Security rules are defined in `.config/project-lint/rules/slices/security-static.toml` with:

- Rule names and descriptions
- Severity levels
- Message templates
- Fix suggestions
- Pattern configurations

## Future Enhancements

- [ ] Parallel file scanning using rayon
- [ ] Incremental scanning (only changed files)
- [ ] Custom rule definitions in TOML
- [ ] Integration with external security tools (semgrep, bandit)
- [ ] Certificate expiration checking
- [ ] Cryptographic key strength validation
- [ ] OWASP Top 10 checks
