# TypeScript Rules

TypeScript rules enforce TypeScript best practices, type safety, and code organization.

## Configuration
```toml
[[modular_rules]]
name = "typescript-standards"
description = "TypeScript best practices and type safety"
enabled = true
severity = "warning"
triggers = ["pre_write_code", "post_read_code"]

[modular_rules.typescript]
enforce_strict_types = true
require_explicit_returns = true
no_any_types = true
prefer_interfaces = true
```

## Rules
- Use `interface` over `type` for object shapes
- Avoid `any` type
- Enforce explicit return types
- Use proper import/export syntax
- Prefer `const` assertions

## Examples
✅ Good: `interface User { name: string }`
✅ Good: `const API_URL = "https://api.example.com" as const`
❌ Bad: `type User = { name: string }`
❌ Bad: `let data: any`
