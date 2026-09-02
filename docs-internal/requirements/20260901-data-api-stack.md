# PRD: Data/API Stack (SQL Migrations, Protobuf, Prisma Schema)

**Date**: 2026-09-01
**Status**: in-progress
**Scope**: New scanners for data and API definition files: SQL
migrations (`*.sql`), Protocol Buffers (`.proto`), and Prisma schema
files (`schema.prisma`). The scan data shows 30+ SQL migration files
in `buzz`, and the user has noted that some open-source projects
require `.proto` files. Prisma is not currently used but is a common
ORM that may be adopted.

## Problem

No data/API definition validation exists in project-lint. SQL
migrations can have numbering conflicts, missing down migrations, or
dangerous operations. Protobuf files can have missing reserved fields
or naming violations. Prisma schemas can have missing indexes or
relation issues.

## File Types Covered

| File type | Count | Scanner |
|-----------|-------|---------|
| `*.sql` (migrations) | ~30+ | sql_migration |
| `*.proto` | 0 (planned) | protobuf_lint |
| `schema.prisma` / `*.prisma` | 0 (planned) | prisma_schema |

## Rules

### sql_migration (check name: `sql_migration`) — NEW SCANNER

#### Migration file rules
- [ ] `sql-migration-sequential-numbering` — Migration files should use sequential numbering (`0001_`, `0002_`, not `001_`, `010_`, `0001_`). **Severity: warning.** Auto-fixable: no.
- [ ] `sql-migration-no-gaps` — Migration numbering should not have gaps (if `0007` exists, `0001`-`0006` must all exist). **Severity: warning.** Auto-fixable: no.
- [ ] `sql-migration-has-description` — Migration filenames should include a description (`0001_initial_schema.sql`, not `0001.sql`). **Severity: info.** Auto-fixable: no.
- [ ] `sql-migration-no-drop-table` — Migrations should not use `DROP TABLE` without a guard (`IF EXISTS`, `CASCADE`). **Severity: error.** Auto-fixable: no.
- [ ] `sql-migration-no-drop-database` — Migrations must not use `DROP DATABASE`. **Severity: error.** Auto-fixable: no.
- [ ] `sql-migration-transactional` — DDL migrations should be wrapped in `BEGIN`/`COMMIT` (or use `CREATE TABLE IF NOT EXISTS` for idempotency). **Severity: warning.** Auto-fixable: no.
- [ ] `sql-migration-no-select-star` — Migrations should not create views with `SELECT *` (explicit column lists are more maintainable). **Severity: info.** Auto-fixable: no.
- [ ] `sql-migration-idempotent` — Migrations should use `IF NOT EXISTS` / `IF EXISTS` guards for idempotency. **Severity: info.** Auto-fixable: no.
- [ ] `sql-migration-no-hardcoded-secrets` — Migrations must not contain hardcoded passwords, tokens, or API keys in `INSERT` statements. **Severity: error.** Auto-fixable: no.

### protobuf_lint (check name: `protobuf_lint`) — NEW SCANNER

#### .proto file rules
- [ ] `proto-syntax-version` — `.proto` files should specify `syntax = "proto3"` (not proto2). **Severity: warning.** Auto-fixable: no.
- [ ] `proto-package-present` — `.proto` files should have `package` declaration. **Severity: error.** Auto-fixable: no.
- [ ] `proto-no-reserved-collision` — Field numbers should not collide with `reserved` numbers. **Severity: error.** Auto-fixable: no.
- [ ] `proto-field-naming` — Field names should use snake_case (`field_name`, not `fieldName`). **Severity: warning.** Auto-fixable: no.
- [ ] `proto-message-naming` — Message names should use PascalCase (`MessageName`, not `message_name`). **Severity: warning.** Auto-fixable: no.
- [ ] `proto-enum-zero-value` — Enum first value should be `0` and named `_UNSPECIFIED` or `_UNKNOWN` (`enum Status { STATUS_UNSPECIFIED = 0; ... }`). **Severity: warning.** Auto-fixable: no.
- [ ] `proto-imports-used` — All `import` statements should be used (no unused imports). **Severity: info.** Auto-fixable: no.
- [ ] `proto-no-deprecated-fields` — `.proto` files should not have `deprecated = true` fields without `reserved` numbers for the field. **Severity: warning.** Auto-fixable: no.

### prisma_schema (check name: `prisma_schema`) — NEW SCANNER

#### schema.prisma rules
- [ ] `prisma-datasource-provider` — `datasource` block should specify `provider` (`postgresql`, `mysql`, `sqlite`). **Severity: error.** Auto-fixable: no.
- [ ] `prisma-datasource-url` — `datasource` block should specify `url` via `env("DATABASE_URL")`, not hardcoded. **Severity: error.** Auto-fixable: no.
- [ ] `prisma-model-id-field` — Models should have an `@id` field. **Severity: error.** Auto-fixable: no.
- [ ] `prisma-model-timestamps` — Models should have `createdAt` and `updatedAt` fields with `@default(now())` and `@updatedAt`. **Severity: info.** Auto-fixable: no.
- [ ] `prisma-relation-index` — Relation fields (`@relation`) should have corresponding `@@index` for foreign key columns. **Severity: warning.** Auto-fixable: no.
- [ `prisma-no-cascade-delete` — Relations should not use `onDelete: Cascade` without careful consideration. **Severity: warning.** Auto-fixable: no.
- [ ] `prisma-generator-client` — `generator` block should specify `provider = "prisma-client-js"` (or equivalent). **Severity: info.** Auto-fixable: no.
- [ ] `prisma-no-hardcoded-secrets` — Schema must not contain hardcoded connection strings or passwords. **Severity: error.** Auto-fixable: no.

## Implementation

### SqlMigrationScanner

Walks project for `*.sql` files in `migrations/` directories. Parses
each as text, checks for `DROP TABLE`, `DROP DATABASE`, `SELECT *`,
`BEGIN`/`COMMIT`, hardcoded secrets. Checks filename numbering across
the migration directory.

### ProtobufLintScanner

Walks project for `*.proto` files. Parses each as text, checks for
`syntax`, `package`, field naming, enum zero values, reserved
collisions using regex.

### PrismaSchemaScanner

Walks project for `*.prisma` files. Parses each as text, checks for
`datasource`, `generator`, `model` blocks, `@id`, `@relation`,
`@@index` using regex.

## Configuration

```toml
[scanner_config.sql_migration]
require_sequential = true
require_idempotent = false
forbid_drop_table = true
forbid_drop_database = true
migration_dirs = ["migrations", "db/migrations", "sql/migrations"]

[scanner_config.protobuf_lint]
require_proto3 = true
require_enum_zero_value = true

[scanner_config.prisma_schema]
require_datasource = true
require_generator = true
require_model_timestamps = false
```

## Acceptance Criteria

- [ ] All three scanners exist with `scan()` returning `Vec<ScannerIssue>`
- [ ] All three registered in `mod.rs`, wired in `lint.rs`, config in `config.rs`, documented in `AGENTS.md`
- [ ] All scanners are SILENT when no matching files exist
- [ ] `SqlMigrationScanner` detects numbering gaps across a migration directory
- [ ] All scanners use centralized exclusion list
- [ ] Tests for each rule
- [ ] Smoke test: `sql_migration` fires on `buzz` (has migrations/ dir)
- [ ] Smoke test: `protobuf_lint` and `prisma_schema` are silent on all current repos
- [ ] `devbox run -- just quality` passes
- [ ] `devbox run -- just quality-full` passes

## Out of Scope

- **SQL execution** — no `psql` or database connection.
- **Protobuf compilation** — no `protoc` execution.
- **Prisma client generation** — no `prisma generate` execution.
- **GraphQL schemas** — `*.graphql` / `schema.graphql` not covered. Future scanner.
- **OpenAPI specs** — `openapi.yaml` / `swagger.json` not covered. Future scanner.
- **gRPC service definitions** — covered indirectly via `.proto` scanner.
- **DBT models** — `*.sql` in `models/` for DBT not covered. Future scanner.

## Dependencies

- **Centralized exclusion list** — must not scan `node_modules/`, `target/`, etc.
