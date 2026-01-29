# Runtime Guard Rules

Runtime guard rules detect browser-specific APIs, security issues, and runtime problems.

## Configuration
```toml
[[modular_rules]]
name = "runtime-guards"
description = "Runtime security and compatibility checks"
enabled = true
severity = "warning"
triggers = ["pre_write_code", "post_read_code"]

[modular_rules.runtime]
check_browser_apis = true
check_nodejs_specific = true
check_eval_usage = true
```

## Guard Categories
- Browser API detection in server code
- Node.js-specific code in browser
- eval() and Function() usage
- DOM manipulation patterns

## Examples
✅ Server: `fs.readFileSync()` (Node.js)
✅ Browser: `document.getElementById()` (Browser)
❌ Server: `document.getElementById()` (Browser API)
❌ Browser: `fs.readFileSync()` (Node.js API)
