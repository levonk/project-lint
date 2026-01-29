# AST Rules

AST (Abstract Syntax Tree) rules perform deep code analysis using tree-sitter for pattern detection.

## Configuration
```toml
[[modular_rules]]
name = "ast-analysis"
description = "Deep code analysis using AST"
enabled = true
severity = "warning"
triggers = ["post_read_code"]

[modular_rules.ast]
languages = ["javascript", "typescript", "python", "rust"]
queries = ["complexity", "security", "patterns"]
```

## Supported Languages
- JavaScript/TypeScript
- Python
- Rust
- JSON
- YAML
- TOML

## Analysis Types
- Complexity metrics
- Security patterns
- Code smells
- Anti-patterns

## Examples
- Detect nested functions (>3 levels)
- Find large functions (>50 lines)
- Identify unused variables
- Spot potential memory leaks
