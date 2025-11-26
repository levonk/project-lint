# TypeScript Linting Module

## Overview

The TypeScript linting module enforces TypeScript and JavaScript code quality standards based on the job-aide typescript-rules.md guidelines. It provides static analysis for file extensions, path aliases, module systems, code style, and package structure.

## Architecture

The TypeScript module uses the generic detection framework (from `src/detection.rs`) to implement language-specific rules:

- **`TypeScriptRuleSet`**: Defines all TypeScript-specific rules organized by category
- **`TypeScriptScanner`**: Orchestrates scanning across multiple rule categories
- **File-type aware scanning**: Only applies relevant rules to appropriate files

## Rules Categories

### 1. File Extension Rules

**Purpose**: Enforce explicit module system indicators

**Rules**:
- ❌ Ambiguous `.ts` files (use `.mts` for ESM, `.cts` for CommonJS)
- ❌ Ambiguous `.js` files (use `.mjs` for ESM, `.cjs` for CommonJS)
- ✅ `.tsx` for React components (always ESM)
- ✅ `.d.ts` for type declarations
- ✅ `.test.ts` for test files (legacy, but allowed)
- ✅ `.config.ts` for config files (legacy, but allowed)

**Severity**: High

**Rationale**: Explicit extensions prevent tooling confusion and clearly indicate whether a file uses ESM or CommonJS, allowing safe mixing of both module systems in the same package.

**Example**:
```typescript
// ❌ Bad
export const foo = () => {};  // in file.ts

// ✅ Good
export const foo = () => {};  // in file.mts (ESM)
// or
module.exports = { foo };     // in file.cts (CommonJS)
```

### 2. Path Alias Rules

**Purpose**: Prevent conflicts with npm scoped packages

**Rules**:
- ❌ Ambiguous `@/*` aliases (conflicts with `@radix-ui/*`, `@testing-library/*`, etc.)
- ✅ Explicit category aliases: `@/core/*`, `@/features/*`, `@/components/*`, `@/utils/*`, `@/lib/*`, `@/types/*`
- ✅ Project-specific prefixes: `@/job-aide/*`, `@/app/*`

**Severity**: High

**Rationale**: Explicit aliases provide clear intent and prevent conflicts with third-party scoped packages.

**Example**:
```json
// ❌ Bad
{
  "compilerOptions": {
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}

// ✅ Good
{
  "compilerOptions": {
    "paths": {
      "@/core/*": ["./src/core/*"],
      "@/features/*": ["./src/features/*"],
      "@/components/*": ["./src/components/*"],
      "@/utils/*": ["./src/utils/*"],
      "@/lib/*": ["./src/lib/*"],
      "@/types/*": ["./src/types/*"]
    }
  }
}
```

### 3. Module System Rules

**Purpose**: Enforce consistent ESM/CommonJS usage

**Rules**:
- ❌ `require()` in ESM files (`.mts`, `.tsx`)
- ❌ `import` in CommonJS files (`.cts`)
- ✅ `import` statements in ESM files
- ✅ `require()` in CommonJS files

**Severity**: High

**Rationale**: Consistent module system prevents runtime errors and improves tooling compatibility.

**Example**:
```typescript
// ❌ Bad in .mts file
const express = require('express');

// ✅ Good in .mts file
import express from 'express';

// ❌ Bad in .cts file
import express from 'express';

// ✅ Good in .cts file
const express = require('express');
```

### 4. Code Style Rules

**Purpose**: Enforce consistent TypeScript code style

**Rules**:
- ✅ Double quotes (`"`) not single quotes (`'`)
- ✅ 2-space indentation
- ✅ Semicolons at end of statements
- ✅ Kebab-case for filenames
- ✅ `type` over `interface` for type definitions
- ✅ `import type` for type-only imports

**Severity**: Medium to Low

**Example**:
```typescript
// ❌ Bad
interface User {
  name: string;
}
import { User } from './types';
const greeting = 'Hello';

// ✅ Good
type User = {
  name: string;
};
import type { User } from './types';
const greeting = "Hello";
```

### 5. ESLint Configuration Rules

**Purpose**: Ensure proper ESLint setup

**Rules**:
- ✅ Use `@job-aide/tools-lint-eslint-config`
- ❌ Direct `process.env` access (use config abstraction)
- ✅ ESLint config file present

**Severity**: Medium

**Example**:
```typescript
// ❌ Bad
const apiKey = process.env.API_KEY;

// ✅ Good
import { config } from './config';
const apiKey = config.apiKey;
```

### 6. Package Structure Rules

**Purpose**: Ensure complete project documentation

**Rules**:
- ✅ `README.md` with usage and examples
- ✅ JSDoc comments on public APIs
- ✅ `docs/` directory for detailed documentation
- ✅ `tests/` directory with test files

**Severity**: Informational

### 7. Test File Rules

**Purpose**: Enforce proper test file configuration

**Rules**:
- ✅ `.test.mts` for ESM test files (not `.test.ts`)
- ✅ `.spec.mts` for ESM spec files
- ✅ Vitest framework configured
- ✅ Tests for all new features

**Severity**: High

**Example**:
```typescript
// ❌ Bad
// file.test.ts
import { describe, it, expect } from 'vitest';

// ✅ Good
// file.test.mts
import { describe, it, expect } from 'vitest';
```

## Usage

### Basic Scanning

```bash
# Run TypeScript linting
project-lint lint

# TypeScript issues will be reported with 📘 indicator:
# 📘 [TypeScript] [HIGH] Ambiguous extension '.ts' detected...
```

### Dry-Run Mode

Preview what would be fixed:

```bash
project-lint lint --dry-run
```

### Auto-Fix Mode

Automatically apply fixes:

```bash
project-lint lint --fix
```

## File Type Support

TypeScript scanning runs on:

- **TypeScript Files**: `.ts`, `.mts`, `.cts`, `.tsx`
- **JavaScript Files**: `.js`, `.mjs`, `.cjs`, `.jsx`
- **Configuration Files**: `tsconfig.json`, `package.json`, `eslint.config.mts`
- **Test Files**: `.test.ts`, `.test.mts`, `.spec.ts`, `.spec.mts`

## Integration with Lint Command

The TypeScript analysis is automatically integrated into the lint command:

1. Loads configuration
2. Determines active profiles
3. Processes modular rules
4. Performs AST analysis
5. Performs security analysis
6. **Performs TypeScript analysis** ← New
7. Performs legacy checks
8. Reports all issues

## Performance Considerations

- **Skipped Directories**: `node_modules`, `dist`, `build`, `.git`
- **Skipped Files**: Minified files (`.min.js`), lock files, hidden files
- **Regex Compilation**: Patterns are compiled once at scanner initialization
- **Selective Scanning**: Only scans TypeScript/JavaScript files

## Configuration

TypeScript rules are defined in:

- `.config/project-lint/rules/slices/typescript.toml`: Rule definitions
- `.config/project-lint/rules/profiles/typescript.toml`: Profile activation and settings

## Extending TypeScript Rules

To add new TypeScript rules:

1. Add a new method to `TypeScriptRuleSet` returning `Vec<PatternRule>` or `Vec<FunctionCallRule>`
2. Create a detector instance in `TypeScriptScanner::new()`
3. Call the detector in `TypeScriptScanner::scan_file()`

Example:

```rust
pub fn new_typescript_rules() -> Vec<PatternRule> {
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

## References

- **job-aide TypeScript Rules**: `/Users/micro/p/gh/lrepo52/job-aide/.windsurf/rules/typescript-rules.md`
- **ADR-20251019001**: File Extension Rationale
- **ADR-20251019002**: Path Alias Safety Guidelines
- **ADR-20251019003**: ESLint Plugin Composition API

## Future Enhancements

- [ ] Parallel file scanning using rayon
- [ ] Incremental scanning (only changed files)
- [ ] Custom rule definitions in TOML
- [ ] Integration with TypeScript compiler API for deeper analysis
- [ ] Automatic migration tools (`.ts` → `.mts`/`.cts`)
- [ ] ESLint rule validation
- [ ] Dependency version checking
- [ ] Performance profiling and optimization
