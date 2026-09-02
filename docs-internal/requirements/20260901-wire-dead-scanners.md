# PRD: Wire Dead Scanners (config_validation, markdown_frontmatter, runtime_guards)

**Date**: 2026-09-01
**Status**: proposed
**Scope**: Retroactive PRD for three scanners that exist in
`project-lint-core/src/scanners/` but are not wired into
`src/commands/lint.rs::run`. This PRD covers the adapter work needed
to bring them to the standard scanner interface, wire them, and make
them production-ready.

## Problem

Three scanners were written but never integrated:

| Scanner | File | Check name | Current API |
|---------|------|------------|-------------|
| Config validation | `config_validation.rs` | `config_validation` | Static methods returning `ConfigViolation` |
| Markdown frontmatter | `markdown_frontmatter.rs` | `markdown_frontmatter` | Static method returning `FrontmatterValidation` |
| Runtime guards | `runtime_guards.rs` | `runtime_guards` | Static methods returning `BrowserAccessViolation` |

All three use a **different API pattern** than the wired scanners. Wired
scanners implement a struct with `pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>>` and are called via
`perform_scanner_issues()`. The dead scanners use static methods that
return custom violation types and operate on file content, not project
paths. They need adapter wrappers before they can be wired.

## File Types Covered

| File type | Count (levonk+lrepo52) | Scanner |
|-----------|----------------------|---------|
| `tsconfig.json` | ~320 | config_validation |
| `eslint.config.mts` / `.ts` / `.js` | ~20 | config_validation |
| `tailwind.config.ts` / `.js` / `.mts` | ~10 | config_validation |
| `package.json` | ~2500 (first-party ~50) | config_validation |
| `*.md` (with frontmatter) | ~4000+ | markdown_frontmatter |
| `*.ts` / `.tsx` / `.mts` / `.js` / `.jsx` | ~3500+ | runtime_guards |

## Rules

### config_validation (check name: `config_validation`)

#### tsconfig.json rules
- [ ] `tsconfig-strict-mode` — `"strict": true` must be present. **Severity: error.** Auto-fixable: no.
- [ ] `tsconfig-module-resolution` — `moduleResolution` must be configured. **Severity: warning.** Auto-fixable: no.
- [ ] `tsconfig-no-ambiguous-alias` — `@/*` path alias is forbidden; use explicit aliases like `@/core/*`, `@/features/*`. **Severity: error.** Auto-fixable: no.
- [ ] `tsconfig-rootdir` — `rootDir` must be configured. **Severity: warning.** Auto-fixable: no.
- [ ] `tsconfig-outdir` — `outDir` must be configured. **Severity: warning.** Auto-fixable: no.

#### eslint.config rules
- [ ] `eslint-config-extension` — ESLint config must be named `eslint.config.mts` (not `.ts` or `.js`). **Severity: error.** Auto-fixable: yes (rename).
- [ ] `eslint-config-base` — Must use `@job-aide/tools-lint-eslint-config` as base config. **Severity: error.** Auto-fixable: no. **Note**: This rule is job-aide-specific. The wired version should make the required base config package configurable via `[scanner_config.config_validation]` so non-job-aide projects can disable or customize it.
- [ ] `eslint-runtime-guards-plugin` — Web projects (detected via `react: true` in config) should include runtime guards plugin. **Severity: warning.** Auto-fixable: no.

#### tailwind.config rules
- [ ] `tailwind-config-extension` — Tailwind config must be `.ts` or `.mts` (not `.js`). **Severity: error.** Auto-fixable: yes (rename).
- [ ] `tailwind-content-present` — `content:` configuration must be present. **Severity: error.** Auto-fixable: no.
- [ ] `tailwind-content-not-empty` — `content:` array must not be empty. **Severity: error.** Auto-fixable: no.

#### package.json rules
- [ ] `package-json-type-field` — `"type"` field must be present (recommend `"module"` for ESM). **Severity: error.** Auto-fixable: no.
- [ ] `package-json-exports-field` — Library packages (those with `"name"` but no `"private": true`) should have `"exports"` field. **Severity: warning.** Auto-fixable: no.
- [ ] `package-json-no-npm-scripts` — Scripts must not call `npm run` or `npm install`. Use pnpm. **Severity: error.** Auto-fixable: no.
- [ ] `package-json-no-yarn-scripts` — Scripts must not call `yarn`. Use pnpm. **Severity: error.** Auto-fixable: no.

### markdown_frontmatter (check name: `markdown_frontmatter`)

#### General markdown rules (all `.md` files)
- [ ] `md-frontmatter-present` — File must start with `---` frontmatter block. **Severity: warning.** Auto-fixable: no.
- [ ] `md-frontmatter-closed` — Frontmatter block must have closing `---`. **Severity: error.** Auto-fixable: no.
- [ ] `md-frontmatter-title` — `title` field is required and non-empty. **Severity: warning.** Auto-fixable: no.
- [ ] `md-frontmatter-synopsis` — `synopsis` field is required and non-empty. **Severity: warning.** Auto-fixable: no.
- [ ] `md-frontmatter-tags` — `tags` field is required and non-empty (not `[]`). **Severity: warning.** Auto-fixable: no.

#### ADR-specific rules (files under `internal-docs/adr/` or `docs-internal/adr/`)
- [ ] `adr-id-required` — ADR files must have `adr-id` field. **Severity: error.** Auto-fixable: no.
- [ ] `adr-id-format` — `adr-id` must match `YYYYMMDDNNN` (11 digits). **Severity: error.** Auto-fixable: no.
- [ ] `adr-status-required` — ADR files must have `status` field. **Severity: error.** Auto-fixable: no.
- [ ] `adr-status-valid` — `status` must be one of: `proposed`, `accepted`, `deprecated`, `superseded`. **Severity: error.** Auto-fixable: no.
- [ ] `adr-date-format` — `date-created` and `date-updated` must match `YYYY-MM-DD`. **Severity: error.** Auto-fixable: no.
- [ ] `adr-version-format` — `version` must be semantic versioning (`X.Y.Z`). **Severity: error.** Auto-fixable: no.

### runtime_guards (check name: `runtime_guards`)

#### TypeScript/JavaScript file rules
- [ ] `runtime-guard-window-access` — Unguarded `window.` access detected. Must import and use `@job-aide/runtime-guards`. **Severity: error.** Auto-fixable: no. **Note**: The required guards package should be configurable via `[scanner_config.runtime_guards]` so non-job-aide projects can customize.
- [ ] `runtime-guard-document-access` — Unguarded `document.` access. **Severity: error.** Auto-fixable: no.
- [ ] `runtime-guard-navigator-access` — Unguarded `navigator.` access. **Severity: error.** Auto-fixable: no.
- [ ] `runtime-guard-localstorage-access` — Unguarded `localStorage.` access. **Severity: error.** Auto-fixable: no.
- [ ] `runtime-guard-sessionstorage-access` — Unguarded `sessionStorage.` access. **Severity: error.** Auto-fixable: no.
- [ ] `runtime-guard-typeof-window` — `typeof window !== "undefined"` pattern detected; use `isBrowser()` from runtime-guards instead. **Severity: warning.** Auto-fixable: no.
- [ ] `runtime-guard-typeof-document` — `typeof document !== "undefined"` pattern; use `isBrowser()` instead. **Severity: warning.** Auto-fixable: no.

## Adapter Work Required

Each scanner needs a wrapper struct that implements the standard
`scan()` interface:

### ConfigValidationScanner
```rust
pub struct ConfigValidationScanner {
    required_eslint_base: Option<String>,  // None = skip eslint base check
    require_type_module: bool,
    check_tailwind: bool,
}

impl ConfigValidationScanner {
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        // Walk project for tsconfig.json, eslint.config.*, tailwind.config.*, package.json
        // Call existing static methods, convert ConfigViolation -> ScannerIssue
    }
}
```

### MarkdownFrontmatterScanner
```rust
pub struct MarkdownFrontmatterScanner {
    require_frontmatter: bool,  // false = only validate if frontmatter present
    adr_dirs: Vec<String>,      // directories to apply ADR-specific rules
}

impl MarkdownFrontmatterScanner {
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        // Walk project for .md files
        // Call existing validate_frontmatter, convert errors -> ScannerIssue
    }
}
```

### RuntimeGuardsScanner
```rust
pub struct RuntimeGuardsScanner {
    guards_package: String,  // default: "@job-aide/runtime-guards"
    check_extensions: Vec<String>,  // default: ts, tsx, mts, js, jsx
}

impl RuntimeGuardsScanner {
    pub fn scan(&self, project_path: &str) -> Result<Vec<ScannerIssue>> {
        // Walk project for TS/JS files
        // Call existing check_unguarded_browser_access, convert violations -> ScannerIssue
    }
}
```

## Configuration

```toml
[scanner_config.config_validation]
required_eslint_base = "@job-aide/tools-lint-eslint-config"  # set to "" to disable
require_type_module = true
check_tailwind = true

[scanner_config.markdown_frontmatter]
require_frontmatter = false  # warn-only if no frontmatter
adr_dirs = ["internal-docs/adr", "docs-internal/adr"]

[scanner_config.runtime_guards]
guards_package = "@job-aide/runtime-guards"
check_extensions = ["ts", "tsx", "mts", "js", "jsx"]
```

## Acceptance Criteria

- [ ] All three scanners have wrapper structs with `scan()` returning `Vec<ScannerIssue>`
- [ ] All three are registered in `project-lint-core/src/scanners/mod.rs`
- [ ] All three are wired into `src/commands/lint.rs::run` with `is_check_enabled` gates
- [ ] All three have config structs in `project-lint-core/src/config.rs` with `ScannerConfig` fields
- [ ] All three are documented in `AGENTS.md` architecture section
- [ ] Existing unit tests still pass (adapter wrappers don't break existing static method tests)
- [ ] New integration tests verify `scan()` returns correct `ScannerIssue` for each rule
- [ ] Smoke test: scanner is silent on repos without matching file types
- [ ] Smoke test: scanner fires on repos with matching file types (e.g., acryl for tsconfig, any repo with .md for frontmatter)
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **JSON schema validation** — this scanner uses string matching, not JSON parsing. A future `json_schema` scanner could do proper schema validation with `serde_json`.
- **YAML parsing** — frontmatter validation uses simple key:value parsing, not a YAML parser. A future enhancement could use `serde_yaml`.
- **AST-based browser access detection** — runtime_guards uses regex, not tree-sitter. The existing `ast.rs` scanner could eventually subsume this.
- **Auto-fixing** — none of these scanners implement `apply_fixes` in this phase. Auto-fixing frontmatter (adding missing fields) and eslint config (renaming) are future enhancements.

## Dependencies

- **Centralized exclusion list** — these scanners must not scan `node_modules/`, `target/`, `dist/`, `.next/`, `.turbo/`. This depends on the centralized exclusion list being available (see separate PRD).
- **package.json first-party filter** — the `package.json` scanner must only check first-party `package.json` files, not the thousands in `node_modules/`. The centralized exclusion list handles this, but the scanner should also skip any `package.json` under a `node_modules/` path as a belt-and-suspenders measure.
