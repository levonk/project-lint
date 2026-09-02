# Data/API Stack Rules

Data/API stack rules validate SQL migration files, Protocol Buffers
definitions, and Prisma schema files for naming conventions, dangerous
operations, idempotency, and security best practices.

## Overview

Data/API rules help identify:
- SQL migration numbering gaps and dangerous operations
- Missing transaction wrappers and idempotency guards
- Hardcoded secrets in migration files
- Protobuf naming violations and missing declarations
- Enum zero-value conventions and reserved field collisions
- Prisma schema missing datasource/generator blocks
- Missing model @id fields and relation indexes
- Hardcoded database connection strings

## Scanners

### SQL Migration (`sql_migration`)

Validates `*.sql` files in `migrations/`, `db/migrations/`, and
`sql/migrations/` directories.

#### Configuration

```toml
[scanner_config.sql_migration]
require_sequential = true
require_idempotent = false
forbid_drop_table = true
forbid_drop_database = true
migration_dirs = ["migrations", "db/migrations", "sql/migrations"]
```

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `sql-migration-no-gaps` | warning | Migration numbering must be sequential with no gaps |
| `sql-migration-has-description` | info | Filename must include description (e.g. `0001_initial_schema.sql`) |
| `sql-migration-no-drop-table` | error | `DROP TABLE` without `IF EXISTS` guard |
| `sql-migration-no-drop-database` | error | `DROP DATABASE` is forbidden |
| `sql-migration-transactional` | warning | DDL with `BEGIN` but no `COMMIT` |
| `sql-migration-no-select-star` | info | Views with `SELECT *` — prefer explicit columns |
| `sql-migration-idempotent` | info | DDL lacking `IF NOT EXISTS` / `BEGIN`/`COMMIT` guards |
| `sql-migration-no-hardcoded-secrets` | error | Hardcoded passwords/tokens in INSERT statements |

#### Examples

Bad:
```sql
DROP TABLE users;
DROP DATABASE production;
INSERT INTO users (password) VALUES ('secret123');
```

Good:
```sql
DROP TABLE IF EXISTS old_table;
CREATE TABLE IF NOT EXISTS users (id INT PRIMARY KEY);
```

### Protobuf Lint (`protobuf_lint`)

Validates `*.proto` files for syntax, package, naming, and enum conventions.

#### Configuration

```toml
[scanner_config.protobuf_lint]
require_proto3 = true
require_enum_zero_value = true
```

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `proto-syntax-version` | warning | Must specify `syntax = "proto3"` |
| `proto-package-present` | error | Must have `package` declaration |
| `proto-field-naming` | warning | Field names must be snake_case |
| `proto-message-naming` | warning | Message names must be PascalCase |
| `proto-enum-zero-value` | warning | Enum first value must be 0 and named `_UNSPECIFIED`/`_UNKNOWN` |
| `proto-no-reserved-collision` | error | Field numbers must not collide with `reserved` numbers |
| `proto-no-deprecated-fields` | warning | Deprecated fields need `reserved` numbers |
| `proto-imports-used` | info | No unused imports |

#### Examples

Bad:
```proto
syntax = "proto2";
message user_data {
  string userId = 1;
}
```

Good:
```proto
syntax = "proto3";
package com.example.user;
message UserData {
  string user_id = 1;
}
```

### Prisma Schema (`prisma_schema`)

Validates `*.prisma` files for datasource, generator, model, and security
conventions.

#### Configuration

```toml
[scanner_config.prisma_schema]
require_datasource = true
require_generator = true
require_model_timestamps = false
```

#### Rules

| Rule | Severity | Description |
|------|----------|-------------|
| `prisma-datasource-provider` | error | `datasource` block must specify `provider` |
| `prisma-datasource-url` | error | `url` must use `env("DATABASE_URL")`, not hardcoded |
| `prisma-model-id-field` | error | Models must have `@id` field |
| `prisma-model-timestamps` | info | Models should have `createdAt` and `updatedAt` |
| `prisma-relation-index` | warning | `@relation` fields need corresponding `@@index` |
| `prisma-no-cascade-delete` | warning | `onDelete: Cascade` should be used with caution |
| `prisma-generator-client` | info | `generator` block must specify `provider` |
| `prisma-no-hardcoded-secrets` | error | No hardcoded connection strings or passwords |

#### Examples

Bad:
```prisma
datasource db {
  provider = "postgresql"
  url = "postgresql://localhost:5432/db"
}
model User {
  email String @unique
}
```

Good:
```prisma
datasource db {
  provider = "postgresql"
  url = env("DATABASE_URL")
}
model User {
  id        Int      @id @default(autoincrement())
  email     String   @unique
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt
  @@index([email])
}
```

## Exclusion List

All three scanners use the centralized exclusion list from
`project-lint-core/src/utils.rs` to skip `node_modules/`, `target/`,
`dist/`, `build/`, `.git/`, and other build artifacts. See the
[security rules](security.md) documentation for the full exclusion list.

## Silent Operation

All three scanners are silent when no matching files exist in the project.
This means repos without SQL migrations, `.proto` files, or `.prisma` files
will not produce any false positives from these scanners.
