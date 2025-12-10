# Configuration File Linting Rules for project-lint

## Overview

Configuration files (`tsconfig.json`, `eslint.config.mts`, `tailwind.config.ts`, etc.) are critical for project setup and code quality. This document identifies statically-detectable rules for validating these configurations.

---

## 1. TypeScript Configuration (`tsconfig.json`)

### High-Priority Rules

#### 1.1 Strict Mode Enforcement
**Rule**: Enforce TypeScript strict mode for type safety

**Detectable Violations**:
- ❌ `"strict": false` explicitly set
- ⚠️  Missing `"strict"` field (defaults to false)
- ❌ Individual strict flags disabled: `noImplicitAny`, `strictNullChecks`, `strictFunctionTypes`, etc.

**Implementation**:
- Parse `tsconfig.json` JSON
- Check `compilerOptions.strict` is `true`
- Validate no strict flags are set to `false`

**Example**:
```json
✅ {
  "compilerOptions": {
    "strict": true
  }
}

❌ {
  "compilerOptions": {
    "strict": false
  }
}

❌ {
  "compilerOptions": {
    "noImplicitAny": false
  }
}
```

---

#### 1.2 Module Resolution Configuration
**Rule**: Enforce consistent module resolution strategy

**Detectable Violations**:
- ❌ `"module"` not set to `"esnext"` or `"es2020"+`
- ❌ `"moduleResolution"` not set to `"bundler"` or `"node"`
- ⚠️  Mismatched module and target settings

**Implementation**:
- Validate `compilerOptions.module` is modern (esnext, es2020+)
- Validate `compilerOptions.moduleResolution` is `bundler` or `node`
- Check consistency between `module` and `target`

**Example**:
```json
✅ {
  "compilerOptions": {
    "module": "esnext",
    "moduleResolution": "bundler",
    "target": "es2020"
  }
}

❌ {
  "compilerOptions": {
    "module": "commonjs",
    "moduleResolution": "classic"
  }
}
```

---

#### 1.3 Path Aliases Validation
**Rule**: Enforce explicit, non-conflicting path aliases

**Detectable Violations**:
- ❌ Ambiguous `@/*` alias (conflicts with npm scoped packages)
- ❌ Overlapping alias patterns
- ⚠️  Alias paths don't exist on disk

**Implementation**:
- Parse `compilerOptions.paths`
- Detect `@/*` pattern
- Check for overlapping patterns
- Validate paths exist (optional, expensive)

**Example**:
```json
✅ {
  "compilerOptions": {
    "paths": {
      "@/core/*": ["./src/core/*"],
      "@/features/*": ["./src/features/*"],
      "@/components/*": ["./src/components/*"]
    }
  }
}

❌ {
  "compilerOptions": {
    "paths": {
      "@/*": ["./src/*"]
    }
  }
}
```

---

#### 1.4 Source and Output Configuration
**Rule**: Enforce proper source and output directories

**Detectable Violations**:
- ❌ `"rootDir"` not set or incorrect
- ❌ `"outDir"` not set or pointing to source
- ❌ `"declarationDir"` missing for library packages
- ⚠️  Output directory not in `.gitignore`

**Implementation**:
- Validate `compilerOptions.rootDir` matches project structure
- Validate `compilerOptions.outDir` is not in source
- Check `compilerOptions.declaration` is `true` for libraries
- Cross-reference with `.gitignore`

**Example**:
```json
✅ {
  "compilerOptions": {
    "rootDir": "./src",
    "outDir": "./dist",
    "declaration": true,
    "declarationDir": "./dist"
  }
}

❌ {
  "compilerOptions": {
    "outDir": "./src"
  }
}
```

---

#### 1.5 Include/Exclude Patterns
**Rule**: Validate include/exclude patterns match project structure

**Detectable Violations**:
- ❌ `include` patterns don't match actual files
- ⚠️  `exclude` missing `node_modules`
- ⚠️  `exclude` missing `dist` or build directories
- ❌ Overly broad patterns (e.g., `**/*`)

**Implementation**:
- Parse `include` and `exclude` arrays
- Validate glob patterns
- Check for common exclusions
- Warn on overly broad patterns

**Example**:
```json
✅ {
  "include": ["src/**/*.ts", "src/**/*.tsx"],
  "exclude": ["node_modules", "dist", "**/*.test.ts"]
}

❌ {
  "include": ["**/*"],
  "exclude": []
}
```

---

### Medium-Priority Rules

#### 1.6 Lib Configuration
**Rule**: Ensure DOM/ES library targets are appropriate

**Detectable Violations**:
- ⚠️  Web project missing `"dom"` in `lib`
- ⚠️  Node project including `"dom"` in `lib`
- ❌ Outdated ES library versions

**Implementation**:
- Check `compilerOptions.lib` array
- Validate against project type (web vs node)
- Warn on outdated ES versions

---

#### 1.7 Emit Configuration
**Rule**: Validate emit settings for proper output

**Detectable Violations**:
- ❌ `"declaration"` false for library packages
- ⚠️  `"sourceMap"` false in development
- ❌ `"removeComments"` true in development

**Implementation**:
- Check `compilerOptions.declaration`
- Validate `compilerOptions.sourceMap`
- Check `compilerOptions.removeComments`

---

## 2. ESLint Configuration (`eslint.config.mts`)

### High-Priority Rules

#### 2.1 Config File Extension
**Rule**: Enforce `.mts` extension for ESLint config

**Detectable Violations**:
- ❌ `eslint.config.ts` (ambiguous extension)
- ❌ `eslint.config.js` (CommonJS)
- ✅ `eslint.config.mts` (ESM, explicit)

**Implementation**:
- Detect config file name
- Validate extension is `.mts`

**Example**:
```
✅ eslint.config.mts
❌ eslint.config.ts
❌ eslint.config.js
```

---

#### 2.2 ESLint Config Package
**Rule**: Enforce use of `@job-aide/tools-lint-eslint-config`

**Detectable Violations**:
- ❌ Custom ESLint config without base
- ❌ Using deprecated ESLint packages
- ⚠️  Not extending `@job-aide/tools-lint-eslint-config`

**Implementation**:
- Parse ESLint config file
- Check for import of `@job-aide/tools-lint-eslint-config`
- Validate config structure

**Example**:
```typescript
✅ import jobAideEslintConfig from "@job-aide/tools-lint-eslint-config";
✅ export default jobAideEslintConfig({ react: true });

❌ export default {
  rules: { /* custom */ }
}
```

---

#### 2.3 Runtime Guards Plugin
**Rule**: Enforce runtime guards for browser safety

**Detectable Violations**:
- ⚠️  Missing runtime guards plugin in web projects
- ❌ Runtime guards plugin not configured
- ❌ Rule not set to `"error"`

**Implementation**:
- Check for runtime guards plugin in config
- Validate rule severity
- Detect unguarded browser API access (via AST)

**Example**:
```typescript
✅ {
  plugins: {
    "job-aide-runtime": runtimeGuardPlugin
  },
  rules: {
    "job-aide-runtime/require-browser-guard": "error"
  }
}

❌ (missing runtime guards)
export default jobAideEslintConfig({ react: true });
```

---

#### 2.4 Rule Severity Levels
**Rule**: Enforce appropriate rule severity levels

**Detectable Violations**:
- ❌ Security rules set to `"warn"` instead of `"error"`
- ❌ Style rules set to `"error"` instead of `"warn"`
- ⚠️  Inconsistent severity across similar rules

**Implementation**:
- Parse rules configuration
- Categorize rules by type (security, style, best-practice)
- Validate severity levels

---

### Medium-Priority Rules

#### 2.5 Ignored Files Configuration
**Rule**: Validate ignored files list

**Detectable Violations**:
- ⚠️  Missing `node_modules` in ignores
- ⚠️  Missing `dist` or build directories
- ⚠️  Missing `.git` directory

**Implementation**:
- Check `ignores` array
- Validate common patterns are present

---

#### 2.6 File Pattern Coverage
**Rule**: Ensure all file types are covered

**Detectable Violations**:
- ⚠️  No rules for `.mts` files
- ⚠️  No rules for `.cts` files
- ⚠️  No rules for `.tsx` files

**Implementation**:
- Parse file patterns in config
- Validate coverage for all extensions

---

## 3. Tailwind CSS Configuration (`tailwind.config.ts`)

### High-Priority Rules

#### 3.1 Config File Extension
**Rule**: Enforce `.ts` extension (or `.mts` for ESM)

**Detectable Violations**:
- ❌ `tailwind.config.js` (CommonJS)
- ⚠️  `tailwind.config.cjs` (explicit CommonJS)
- ✅ `tailwind.config.ts` or `tailwind.config.mts`

**Implementation**:
- Detect config file name
- Validate extension

---

#### 3.2 Content Configuration
**Rule**: Enforce content patterns for proper purging

**Detectable Violations**:
- ❌ Missing `content` field
- ❌ Empty `content` array
- ⚠️  Overly broad patterns (e.g., `**/*`)
- ⚠️  Patterns don't match actual files

**Implementation**:
- Parse `content` array
- Validate patterns are not empty
- Check for common file extensions

**Example**:
```typescript
✅ {
  content: [
    './app/**/*.{js,jsx,ts,tsx}',
    './components/**/*.{js,jsx,ts,tsx}'
  ]
}

❌ {
  content: []
}

❌ (missing content)
export default { theme: { extend: {} } }
```

---

#### 3.3 Theme Configuration
**Rule**: Validate theme structure

**Detectable Violations**:
- ⚠️  Empty `theme.extend` (no customization)
- ⚠️  Overriding core theme without reason
- ❌ Invalid color values or spacing units

**Implementation**:
- Parse `theme` object
- Validate color/spacing values
- Warn on empty extends

---

#### 3.4 Plugins Configuration
**Rule**: Validate Tailwind plugins

**Detectable Violations**:
- ⚠️  Empty `plugins` array
- ❌ Using deprecated plugins
- ⚠️  Plugin configuration missing required options

**Implementation**:
- Check `plugins` array
- Validate plugin names
- Cross-reference with package.json

---

### Medium-Priority Rules

#### 3.5 Preset Configuration
**Rule**: Validate Tailwind presets

**Detectable Violations**:
- ⚠️  Using multiple conflicting presets
- ❌ Preset not found in dependencies

**Implementation**:
- Parse `presets` array
- Validate preset packages exist

---

#### 3.6 CSS Directives
**Rule**: Validate CSS file directives

**Detectable Violations**:
- ⚠️  Missing `@tailwind` directives
- ❌ Incorrect directive order
- ⚠️  Custom directives in wrong file

**Implementation**:
- Scan CSS files for Tailwind directives
- Validate order: `base`, `components`, `utilities`

---

## 4. Package Configuration (`package.json`)

### High-Priority Rules

#### 4.1 Type Field
**Rule**: Enforce explicit module type

**Detectable Violations**:
- ⚠️  Missing `"type"` field
- ❌ `"type": "commonjs"` in ESM-only packages
- ❌ Mismatch between `"type"` and file extensions

**Implementation**:
- Check `type` field presence
- Validate against file extensions
- Warn on ambiguous setup

**Example**:
```json
✅ {
  "type": "module",
  "exports": {
    ".": "./dist/index.mjs"
  }
}

❌ (missing type)
{
  "main": "./dist/index.js"
}
```

---

#### 4.2 Exports Field
**Rule**: Enforce proper exports configuration

**Detectable Violations**:
- ⚠️  Missing `exports` field in libraries
- ❌ Exports pointing to source instead of dist
- ❌ Inconsistent export paths

**Implementation**:
- Check `exports` field
- Validate paths point to dist
- Check for consistency

---

#### 4.3 Scripts Configuration
**Rule**: Validate npm scripts

**Detectable Violations**:
- ⚠️  Missing `build` script
- ⚠️  Missing `test` script
- ❌ Using `npm` or `yarn` instead of `pnpm`
- ⚠️  Scripts not using workspace references

**Implementation**:
- Parse `scripts` object
- Check for required scripts
- Detect npm/yarn usage
- Validate workspace references

**Example**:
```json
✅ {
  "scripts": {
    "build": "tsc",
    "test": "vitest",
    "lint": "eslint ."
  }
}

❌ {
  "scripts": {
    "build": "npm run build"
  }
}
```

---

#### 4.4 Dependencies Configuration
**Rule**: Validate dependency declarations

**Detectable Violations**:
- ⚠️  Duplicate dependencies in `dependencies` and `devDependencies`
- ❌ Missing peer dependency declarations
- ⚠️  Using `*` or `latest` as version specifiers

**Implementation**:
- Parse dependencies
- Check for duplicates
- Validate version specifiers
- Cross-reference peer dependencies

---

### Medium-Priority Rules

#### 4.5 Workspace Configuration
**Rule**: Validate workspace setup

**Detectable Violations**:
- ⚠️  Missing `workspaces` field in monorepo root
- ❌ Workspace patterns don't match actual packages

**Implementation**:
- Check `workspaces` field
- Validate glob patterns

---

## Implementation Roadmap

### Phase 1: Core Config Validation (Weeks 1-2)
1. TypeScript strict mode enforcement
2. ESLint config package validation
3. Tailwind content configuration
4. Package.json type field

### Phase 2: Advanced Validation (Weeks 3-4)
5. Path aliases validation
6. Runtime guards plugin detection
7. Exports field validation
8. Scripts configuration

### Phase 3: Cross-File Validation (Weeks 5+)
9. Consistency checks across configs
10. Dependency resolution
11. Build output validation

---

## Generic Detection Framework Usage

- **`PatternDetector`**: For JSON/TOML structure validation
- **`FunctionCallDetector`**: For detecting imports and function calls
- **Custom Detectors**: For complex config validation logic

---

## Configuration Files

Each rule set would have:

1. **Slice Definition**: `.config/project-lint/rules/slices/config-validation.toml`
2. **Profile Activation**: `.config/project-lint/rules/profiles/config-validation.toml`
3. **Documentation**: `docs-internal/config-file-rules.md` (this file)

---

## References

- TypeScript: https://www.typescriptlang.org/tsconfig
- ESLint: https://eslint.org/docs/latest/use/configure/
- Tailwind CSS: https://tailwindcss.com/docs/configuration
- npm: https://docs.npmjs.com/cli/v10/configuring-npm/package-json
