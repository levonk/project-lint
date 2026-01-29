# Package Organization Rules

Package organization rules ensure proper structure and organization of packages, modules, and dependencies.

## Overview

Package organization rules help maintain:
- Consistent directory structure
- Proper package placement
- Clean dependency management
- Avoidance of circular dependencies

## Configuration

Create `.config/project-lint/rules/active/package-organization.toml`:

```toml
[[modular_rules]]
name = "package-organization-enforcer"
description = "Enforces proper package organization"
enabled = true
severity = "warning"
triggers = ["post_read_code"]

[modular_rules.file_mappings]
# Map packages to appropriate directories
"packages/*" = "packages/"
"apps/*" = "apps/"
"tools/*" = "tools/"
"libs/*" = "libs/"
```

## Monorepo Structure

### Recommended Layout
```
project/
├── packages/           # Shared packages
│   ├── active/        # Active packages
│   │   ├── core/
│   │   ├── utils/
│   │   └── types/
│   └── icebox/        # Experimental packages
├── apps/              # Applications
│   ├── active/        # Active applications
│   │   ├── web/
│   │   ├── mobile/
│   │   └── desktop/
│   └── icebox/        # Prototype applications
├── tools/             # Development tools
│   ├── build/
│   ├── deploy/
│   └── lint/
├── docs/              # Documentation
├── configs/           # Configuration files
└── scripts/           # Build and utility scripts
```

## Package Categories

### 1. Core Packages (`packages/active/core/`)
Essential packages used across the project:
- Authentication
- Database connections
- Core utilities
- Type definitions

### 2. Feature Packages (`packages/active/features/`)
Domain-specific functionality:
- User management
- Payment processing
- Analytics
- Notifications

### 3. Utility Packages (`packages/active/utils/`)
Reusable utilities:
- Date helpers
- String manipulation
- File operations
- Validation helpers

### 4. Type Packages (`packages/active/types/`)
Shared type definitions:
- API types
- Database schemas
- Event types
- Configuration types

## Custom Rules

### Package Name Validation
```toml
[[rules.custom_rules]]
name = "package-name-convention"
pattern = "packages/*"
check_content = true
content_pattern = "[A-Z][a-zA-Z0-9]*"
severity = "warning"
message = "Package names should use kebab-case (e.g., my-package)"
```

### Dependency Validation
```toml
[[rules.custom_rules]]
name = "no-circular-deps"
pattern = "packages/*/package.json"
check_content = true
content_pattern = "\"name\": \"@workspace/.*\""
severity = "error"
message = "Use workspace dependencies for internal packages"
```

### Index File Requirements
```toml
[[rules.custom_rules]]
name = "require-index-file"
pattern = "packages/*/src/"
required_if_path_exists = "packages/*/package.json"
severity = "warning"
message = "Packages should have an index.ts file for clean exports"
```

## Language-Specific Rules

### TypeScript/JavaScript
```toml
[[modular_rules]]
name = "typescript-package-structure"
description = "TypeScript package organization rules"
enabled = true
severity = "warning"
triggers = ["post_read_code"]

[modular_rules.file_mappings]
"packages/*/src/*.ts" = "packages/*/src/"
"packages/*/src/*.tsx" = "packages/*/src/"
"packages/*/src/*.d.ts" = "packages/*/src/types/"
```

#### Package Structure
```
my-package/
├── src/
│   ├── index.ts          # Main export file
│   ├── types/            # Type definitions
│   │   ├── index.ts
│   │   └── api.types.ts
│   ├── utils/            # Utility functions
│   │   ├── index.ts
│   │   └── helper.ts
│   ├── components/       # React components (if applicable)
│   │   ├── index.ts
│   │   └── MyComponent.tsx
│   └── hooks/            # Custom hooks (if applicable)
│       ├── index.ts
│       └── useMyHook.ts
├── tests/                # Test files
│   ├── unit/
│   └── integration/
├── package.json
├── tsconfig.json
├── README.md
└── .npmignore
```

### Python
```toml
[[modular_rules]]
name = "python-package-structure"
description = "Python package organization rules"
enabled = true
severity = "warning"
triggers = ["post_read_code"]

[modular_rules.file_mappings]
"packages/*/src/*.py" = "packages/*/src/"
"packages/*/tests/*.py" = "packages/*/tests/"
```

#### Package Structure
```
my_package/
├── src/
│   ├── __init__.py       # Main package file
│   ├── types.py          # Type definitions
│   ├── utils.py          # Utility functions
│   ├── api.py            # API clients
│   └── models.py         # Data models
├── tests/
│   ├── __init__.py
│   ├── test_utils.py
│   └── test_api.py
├── pyproject.toml
├── README.md
└── .gitignore
```

### Rust
```toml
[[modular_rules]]
name = "rust-package-structure"
description = "Rust package organization rules"
enabled = true
severity = "warning"
triggers = ["post_read_code"]

[modular_rules.file_mappings]
"packages/*/src/*.rs" = "packages/*/src/"
"packages/*/tests/*.rs" = "packages/*/tests/"
```

#### Package Structure
```
my-package/
├── src/
│   ├── lib.rs           # Main library file
│   ├── types.rs         # Type definitions
│   ├── utils.rs         # Utility functions
│   ├── api.rs           # API clients
│   └── models.rs        # Data models
├── tests/
│   ├── integration_tests.rs
│   └── utils_tests.rs
├── Cargo.toml
├── README.md
└── .gitignore
```

## Workspace Configuration

### pnpm Workspace
```toml
# pnpm-workspace.yaml
packages:
  - 'packages/*'
  - 'apps/*'
  - 'tools/*'
```

### npm Workspace
```json
{
  "name": "my-monorepo",
  "private": true,
  "workspaces": [
    "packages/*",
    "apps/*",
    "tools/*"
  ]
}
```

### Cargo Workspace
```toml
# Cargo.toml (root)
[workspace]
members = [
    "packages/*",
    "apps/*",
    "tools/*"
]
```

## Dependency Management

### Internal Dependencies
Use workspace dependencies for internal packages:

#### TypeScript
```json
{
  "dependencies": {
    "@workspace/core": "workspace:*",
    "@workspace/utils": "workspace:*"
  }
}
```

#### Python
```toml
# pyproject.toml
[project]
dependencies = [
    "core-package @ {root}/packages/core",
    "utils-package @ {root}/packages/utils"
]
```

#### Rust
```toml
# Cargo.toml
[dependencies]
core-package = { path = "../core" }
utils-package = { path = "../utils" }
```

### External Dependencies
- Pin versions for production dependencies
- Use ranges for development dependencies
- Document purpose of each major dependency

## Examples

### Good Package Organization
```
project/
├── packages/
│   ├── active/
│   │   ├── core/
│   │   │   ├── src/
│   │   │   │   ├── index.ts
│   │   │   │   ├── auth/
│   │   │   │   └── database/
│   │   │   └── package.json
│   │   ├── utils/
│   │   │   ├── src/
│   │   │   │   ├── index.ts
│   │   │   │   ├── date.ts
│   │   │   │   └── string.ts
│   │   │   └── package.json
│   │   └── types/
│   │       ├── src/
│   │       │   ├── index.ts
│   │       │   ├── api.types.ts
│   │       │   └── user.types.ts
│   │       └── package.json
│   └── icebox/
│       └── experimental-feature/
├── apps/
│   ├── active/
│   │   ├── web/
│   │   │   ├── src/
│   │   │   ├── package.json
│   │   │   └── vite.config.ts
│   │   └── mobile/
│   │       ├── src/
│   │       ├── package.json
│   │       └── capacitor.config.ts
│   └── icebox/
└── tools/
    ├── build/
    ├── deploy/
    └── lint/
```

### Bad Package Organization
```
project/
├── src/                    # Mixed packages
├── lib/                    # Unclear purpose
├── components/             # Should be in a package
├── utils/                  # Should be a package
├── api/                    # Should be a package
└── random-folder/          # Poor naming
```

## Integration

### Pre-commit Hooks
```bash
#!/bin/bash
# Check package organization
project-lint lint --fix --dry-run
if [ $? -ne 0 ]; then
  echo "Package organization issues found"
  exit 1
fi
```

### CI/CD Integration
```yaml
# .github/workflows/lint.yml
- name: Check package organization
  run: |
    project-lint lint --fix --dry-run
    project-lint logs --stats
```

## Troubleshooting

### Common Issues
1. **Circular Dependencies**: Use dependency graph analysis
2. **Missing Index Files**: Ensure each package has proper exports
3. **Incorrect Workspace Config**: Verify workspace configuration
4. **Version Conflicts**: Use consistent versioning strategy

### Performance Optimization
- Use specific file patterns instead of wildcards
- Exclude large directories from scanning
- Cache dependency analysis results
