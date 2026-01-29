# File Naming Rules

File naming rules enforce consistent naming conventions across your project.

## Overview

File naming rules help maintain:
- Consistent file naming patterns
- Proper file extensions
- Descriptive and meaningful names
- Avoidance of temporary or backup files in commits

## Configuration

Create `.config/project-lint/rules/active/file-naming.toml`:

```toml
[[modular_rules]]
name = "file-naming-enforcer"
description = "Enforces consistent file naming conventions"
enabled = true
severity = "warning"
triggers = ["pre_write_code", "post_read_code"]

[modular_rules.file_mappings]
# Map problematic files to appropriate directories
"*.tmp" = "temp/"
"*.bak" = "backups/"
"*.old" = "backups/"
"Dockerfile.tmp" = "docker/temp/"
```

## Built-in Rules

### Standard File Names
The tool recognizes these standard file names:
- `README.md` - Project documentation
- `LICENSE` - License file
- `CHANGELOG.md` - Change log
- `CONTRIBUTING.md` - Contribution guidelines
- `.gitignore` - Git ignore file
- `package.json` - Node.js package file
- `Cargo.toml` - Rust package file
- `pyproject.toml` - Python project file

### Case Sensitivity
- **Recommended**: kebab-case for files (e.g., `my-component.ts`)
- **Allowed**: snake_case for Python files (e.g., `my_module.py`)
- **Forbidden**: spaces and special characters in filenames

### File Extensions
Enforces proper file extensions:
- `.ts` - TypeScript files
- `.js` - JavaScript files
- `.py` - Python files
- `.rs` - Rust files
- `.md` - Markdown files
- `.yml`/`.yaml` - YAML files
- `.json` - JSON files

## Custom Rules

### Forbidden Patterns
```toml
[[rules.custom_rules]]
name = "no-temp-files"
pattern = "*.tmp"
severity = "warning"
message = "Temporary files should not be committed"
```

### Required Files
```toml
[[rules.custom_rules]]
name = "require-readme"
pattern = "README.md"
required = true
severity = "error"
message = "Every project should have a README.md file"
```

### Case Sensitivity Rules
```toml
[[rules.custom_rules]]
name = "kebab-case-files"
pattern = "*"
check_content = true
content_pattern = "[A-Z][a-zA-Z0-9]*\\.[a-z]+"
severity = "warning"
message = "Use kebab-case for filenames (e.g., my-file.js)"
```

## File Organization

### Recommended Structure
```
project/
├── src/
│   ├── components/
│   │   ├── my-component.ts
│   │   └── another-component.ts
│   ├── utils/
│   │   ├── file-utils.ts
│   │   └── string-utils.ts
│   └── types/
│       └── user-types.ts
├── docs/
│   ├── api.md
│   └── setup.md
├── tests/
│   ├── unit/
│   │   └── file-utils.test.ts
│   └── integration/
│       └── api.test.ts
└── scripts/
    ├── build.sh
    └── deploy.sh
```

### File Naming Patterns

#### TypeScript/JavaScript
- Components: `kebab-case.ts` (e.g., `user-profile.ts`)
- Utilities: `kebab-case.ts` (e.g., `date-utils.ts`)
- Types: `kebab-case.types.ts` (e.g., `user.types.ts`)
- Tests: `kebab-case.test.ts` (e.g., `user-profile.test.ts`)
- Stories: `kebab-case.stories.ts` (e.g., `button.stories.ts`)

#### Python
- Modules: `snake_case.py` (e.g., `file_utils.py`)
- Tests: `test_snake_case.py` (e.g., `test_file_utils.py`)
- Packages: `kebab-case/` (e.g., `my-package/`)

#### Rust
- Modules: `snake_case.rs` (e.g., `file_utils.rs`)
- Tests: `snake_case_tests.rs` (e.g., `file_utils_tests.rs`)
- Examples: `snake_case_example.rs` (e.g., `file_utils_example.rs`)

## Integration with IDE

### VS Code
Add to `.vscode/settings.json`:
```json
{
  "files.insertFinalNewline": true,
  "files.trimTrailingWhitespace": true,
  "files.associations": {
    "*.tmp": "plaintext"
  }
}
```

### Pre-commit Hook
```bash
#!/bin/bash
# Check file naming before commit
project-lint lint --fix --dry-run
if [ $? -ne 0 ]; then
  echo "File naming issues found. Fix before committing."
  exit 1
fi
```

## Examples

### Good Names
- `user-profile.component.ts`
- `date-utils.service.ts`
- `api-client.types.ts`
- `authentication.middleware.ts`
- `database.connection.ts`

### Bad Names
- `UserProfile.ts` (PascalCase)
- `dateUtils.ts` (camelCase)
- `file.js~` (tilde suffix)
- `temp file.txt` (space in name)
- `API_CLIENT.ts` (all caps)

## Troubleshooting

### False Positives
If legitimate files are flagged:
1. Add exception pattern to rule
2. Move file to appropriate directory
3. Use `.project-lint-ignore` file

### Performance Issues
For large projects:
1. Use specific patterns instead of `*`
2. Exclude directories like `node_modules/`
3. Use triggers selectively

### Integration Issues
If rules aren't triggering:
1. Check file permissions
2. Verify trigger events match workflow
3. Enable debug logging
