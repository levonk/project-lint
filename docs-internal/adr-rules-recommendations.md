# Recommended Rules from job-aide ADRs for project-lint

## Overview

This document identifies statically-detectable rules from job-aide's Architecture Decision Records (ADRs) that would be valuable additions to project-lint. These rules enforce architectural decisions and best practices across the monorepo.

## High-Priority Rules (Easy to Implement)

### 1. **Package Organization Structure** (ADR 002)
**Status**: ⭐ High Priority - Highly Detectable

**Rule**: Enforce platform-first package hierarchy
```
packages/{category}/{platform}/{domain}/{package-name}/{language}
```

**Detectable Violations**:
- ❌ Packages not following the structure
- ❌ Missing platform level (`web`, `node`, `shared`)
- ❌ Incorrect category placement

**Implementation**:
- Create a `package-organization` slice
- Use glob patterns to validate directory structure
- Check `package.json` location against expected path

**Example**:
```
✅ packages/features/web/auth/auth-ui/typescript/package.json
❌ packages/features/auth/auth-ui/typescript/package.json (missing platform)
❌ packages/auth/auth-ui/typescript/package.json (wrong structure)
```

---

### 2. **Markdown Frontmatter Standardization** (ADR 20251106016)
**Status**: ⭐ High Priority - Highly Detectable

**Rule**: Enforce standardized YAML frontmatter on all `.md` files

**Required Fields**:
- `title`: Human-readable title
- `synopsis`: One-sentence summary
- `tags`: Array of tags

**ADR-Specific Fields**:
- `adr-id`: Unique identifier (YYYYMMDDNNN)
- `status`: proposed|accepted|deprecated|superseded
- `author`: GitHub URL
- `date-created`: YYYY-MM-DD
- `date-updated`: YYYY-MM-DD
- `version`: Semantic version

**Detectable Violations**:
- ❌ Missing frontmatter block
- ❌ Missing required fields
- ❌ Invalid YAML syntax
- ❌ Invalid date format
- ❌ Invalid status values
- ❌ Missing ADR fields in `internal-docs/adr/` files

**Implementation**:
- Create a `markdown-frontmatter` slice
- Use regex to detect frontmatter blocks
- Validate YAML structure and required fields
- File-type specific rules (ADRs vs general docs)

**Example**:
```markdown
✅ 
---
title: "My Document"
synopsis: "A brief summary"
tags: ["doc", "example"]
adr-id: 20251126001
status: "accepted"
author: "https://github.com/levonk"
date-created: 2025-11-26
date-updated: 2025-11-26
version: 1.0.0
---

❌ (missing frontmatter)
# My Document
```

---

### 3. **pnpm Lockfile Enforcement** (ADR 20251106001)
**Status**: ⭐ High Priority - Highly Detectable

**Rule**: Enforce pnpm as the only package manager

**Detectable Violations**:
- ❌ `package-lock.json` present (npm)
- ❌ `bun.lock` or `bun.lockb` present (bun)
- ❌ `yarn.lock` present (yarn)
- ⚠️  Missing `pnpm-lock.yaml`
- ❌ `npm` or `yarn` commands in scripts

**Implementation**:
- Create a `package-manager` slice
- Detect forbidden lockfiles
- Check `package.json` scripts for npm/yarn commands
- Validate `pnpm-lock.yaml` presence

**Example**:
```
✅ pnpm-lock.yaml exists
❌ package-lock.json exists (npm)
❌ bun.lock exists (bun)
```

---

### 4. **Runtime Guards for Browser Safety** (ADR 006)
**Status**: ⭐ Medium Priority - Moderately Detectable

**Rule**: Enforce use of `@job-aide/runtime-guards` for browser/server checks

**Detectable Violations**:
- ❌ Direct `typeof window !== "undefined"` checks
- ❌ Direct `typeof document !== "undefined"` checks
- ❌ Unguarded `window.` access
- ❌ Unguarded `document.` access
- ⚠️  Missing import of `isBrowser`, `assertBrowser`, or `assertServer`

**Implementation**:
- Create a `runtime-guards` slice
- Use regex to detect unguarded browser API access
- Check for proper imports from `@job-aide/runtime-guards`
- File-type specific (only web TypeScript files)

**Example**:
```typescript
❌ if (typeof window !== "undefined") { /* ... */ }
❌ const el = document.getElementById("app");

✅ import { isBrowser } from "@job-aide/runtime-guards";
✅ if (isBrowser()) { /* ... */ }
```

---

### 5. **Nx Configuration** (ADR-20260419001, supersedes ADR 20251106001)
**Status**: ⭐ Medium Priority - Moderately Detectable

**Rule**: Enforce Nx configuration in monorepo (Turborepo was superseded by ADR-20260419001)

**Detectable Violations**:
- ❌ Missing `nx.json` in root
- ❌ Invalid `nx.json` syntax
- ⚠️  Missing cache configuration
- ⚠️  Missing target defaults

**Implementation**:
- Create a `nx-config` slice
- Validate `nx.json` presence and structure
- Check for essential target defaults (build, lint, test)

**Example**:
```json
✅ nx.json with proper targetDefaults configuration
❌ Missing nx.json
```

---

## Medium-Priority Rules (Moderate Implementation)

### 6. **Dependency Consistency in Monorepo**
**Status**: 🔶 Medium Priority - Requires Dependency Analysis

**Rule**: Enforce consistent dependency versions across monorepo

**Detectable Violations**:
- ⚠️  Same package with different versions in different `package.json` files
- ⚠️  Peer dependency mismatches
- ❌ Undeclared dependencies (phantom dependencies)

**Implementation**:
- Create a `dependency-consistency` slice
- Parse all `package.json` files
- Compare versions across workspace
- Validate peer dependencies

---

### 7. **Platform Boundary Enforcement**
**Status**: 🔶 Medium Priority - Requires Import Analysis

**Rule**: Prevent cross-platform imports (e.g., Node.js code in web packages)

**Detectable Violations**:
- ❌ Web package importing from `packages/.../node/...`
- ❌ Node package importing from `packages/.../web/...`
- ❌ Browser API usage in Node.js code

**Implementation**:
- Create a `platform-boundaries` slice
- Analyze import statements
- Validate against package path structure
- Use AST analysis for browser API detection

---

## Lower-Priority Rules (Complex Implementation)

### 8. **Test Coverage Requirements**
**Status**: 🔴 Lower Priority - Requires Coverage Analysis

**Rule**: Enforce minimum test coverage thresholds

**Detectable Violations**:
- ⚠️  Missing test files for new features
- ⚠️  Coverage below threshold

**Implementation**:
- Integrate with coverage tools (Vitest, Istanbul)
- Parse coverage reports
- Validate test file existence

---

### 9. **Documentation Completeness**
**Status**: 🔴 Lower Priority - Requires Content Analysis

**Rule**: Enforce documentation standards

**Detectable Violations**:
- ❌ Missing `README.md` in packages
- ❌ Missing JSDoc on public APIs
- ❌ Incomplete API documentation

**Implementation**:
- Create a `documentation` slice
- Check for required documentation files
- Parse JSDoc comments
- Validate documentation completeness

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
1. ✅ Package Organization Structure (ADR 002)
2. ✅ Markdown Frontmatter (ADR 20251106016)
3. ✅ pnpm Lockfile Enforcement (ADR 20251106001)

### Phase 2: Safety & Quality (Weeks 3-4)
4. Runtime Guards for Browser Safety (ADR 006)
5. Nx Configuration (ADR-20260419001)

### Phase 3: Advanced (Weeks 5+)
6. Dependency Consistency
7. Platform Boundary Enforcement
8. Test Coverage Requirements
9. Documentation Completeness

---

## Generic Detection Framework Usage

All these rules can be implemented using the existing generic detection framework:

- **`PatternDetector`**: For regex-based detection (frontmatter, lockfiles, imports)
- **`FunctionCallDetector`**: For detecting specific function calls (browser APIs)
- **Custom Detectors**: For complex logic (package structure, dependency analysis)

---

## Configuration Files

Each rule set would have:

1. **Slice Definition**: `.config/project-lint/rules/slices/{rule-name}.toml`
   - Rule definitions
   - Severity levels
   - Message templates

2. **Profile Activation**: `.config/project-lint/rules/profiles/{profile-name}.toml`
   - When to activate the rule
   - Context-specific settings

3. **Documentation**: `docs-internal/{rule-name}-rules.md`
   - Detailed explanation
   - Examples
   - Rationale

---

## References

- **ADR 002**: Refined Package Organization
- **ADR 006**: Runtime Guards for Browser Safety
- **ADR 20251106001**: pnpm (Turborepo portion superseded by ADR-20260419001)
- **ADR-20260419001**: Nx monorepo build tool (supersedes Turborepo)
- **ADR 20251106016**: Standardized Markdown Frontmatter
