# Git Rules

Git rules enforce proper branch naming, commit standards, and repository hygiene.

## Configuration
```toml
[[modular_rules]]
name = "git-standards"
description = "Enforce git standards and best practices"
enabled = true
severity = "warning"
triggers = ["pre_commit", "pre_push"]

[modular_rules.git]
enforce_branch_naming = true
enforce_commit_format = true
require_ticket_reference = true
```

## Rules
- Branch naming: `feature/description`, `bugfix/description`, `hotfix/description`
- Commit format: `type(scope): description`
- No direct pushes to main/master
- Require PR for changes

## Examples
✅ Good: `feature/user-authentication`
✅ Good: `fix: resolve login issue`
❌ Bad: `stuff`
❌ Bad: `master` (use `main`)
