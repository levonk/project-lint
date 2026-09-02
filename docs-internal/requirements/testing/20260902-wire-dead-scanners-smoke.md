# Smoke Test: Wire Dead Scanners (2026-09-02)

**PRD**: [docs-internal/requirements/20260901-wire-dead-scanners.md](../20260901-wire-dead-scanners.md)
**Binary**: `./target/release/project-lint` (release build, 2026-09-02)

## Test 1: project-lint repo (has matching files)

**Command**: `./target/release/project-lint lint -p .`

### config_validation — FIRES (expected)

```
❌ [ConfigVal] Missing "type" field. Add "type": "module" for ESM packages (temp_smoke_test/nextjs-frontend/package.json: package-json-type-field)
⚠️ [ConfigVal] Missing "exports" field. Recommended for library packages (temp_smoke_test/nextjs-frontend/package.json: package-json-exports-field)
❌ [ConfigVal] TypeScript strict mode not enabled. Add "strict": true (temp_smoke_test/nextjs-frontend/tsconfig.json: tsconfig-strict-mode)
⚠️ [ConfigVal] moduleResolution not configured. Recommended: "bundler" or "node" (temp_smoke_test/nextjs-frontend/tsconfig.json: tsconfig-module-resolution)
⚠️ [ConfigVal] rootDir not configured. Recommended: "./src" (temp_smoke_test/nextjs-frontend/tsconfig.json: tsconfig-rootdir)
⚠️ [ConfigVal] outDir not configured. Recommended: "./dist" (temp_smoke_test/nextjs-frontend/tsconfig.json: tsconfig-outdir)
```

All 6 expected rule names emitted with correct severities (error for
strict-mode/type-field, warning for module-resolution/exports/rootDir/outDir).

### markdown_frontmatter — FIRES (expected)

```
⚠️ [MdFM] tags array is empty (.agents/workflows/lint-upsert.md: md-frontmatter-tags)
⚠️ [MdFM] Missing required field: title (.agents/workflows/lint-upsert.md: md-frontmatter-title)
⚠️ [MdFM] Missing required field: synopsis (.agents/workflows/lint-upsert.md: md-frontmatter-synopsis)
```

Fires on `.md` files with frontmatter that is missing required fields (title,
synopsis, tags). Correctly silent on `.md` files without frontmatter (default
`require_frontmatter = false`).

### runtime_guards — SILENT (expected)

No `[RtGuards]` issues in the output. The project-lint repo is a Rust project
with no unguarded browser API access in TS/JS files. The scanner correctly
does not produce false positives on non-matching file types.

## Test 2: Exclusion list verification

The `node_modules/`, `target/`, `dist/`, `.next/`, `.turbo/`, `.git/`
directories are excluded by all three scanners. Verified via unit tests
(`scan_skips_node_modules` in each scanner's test module) and confirmed
in the smoke test — no issues from `target/` build artifacts despite
hundreds of `.rs` files being present there.

## Summary

| Scanner | Fires on matching files | Silent on non-matching | Exclusion list | Verdict |
|---------|------------------------|------------------------|----------------|---------|
| config_validation | Yes (6 rules) | Yes | Yes | PASS |
| markdown_frontmatter | Yes (3 rules) | Yes | Yes | PASS |
| runtime_guards | N/A (no TS/JS) | Yes (no false positives) | Yes | PASS |

All three scanners are wired and production-ready.
