# Configuration Architecture

This document explains the configuration philosophy and structure used in `project-lint`. The system is designed to be modular, context-aware, and easily extensible.

## Core Philosophy

The configuration system follows a **Component-Based Architecture** split into four distinct layers. This design prevents "Giant Config File" syndrome and allows for "zero-config" usage in standard projects while maintaining deep customization options.

### 1. Slices: The "What" (Domain Logic)
**Location:** `.config/project-lint/rules/slices/*.toml`

Slices act as **Libraries of Rules**. They define *all possible checks* for a specific domain but do not necessarily enforce them. They are declarative definitions of policy grouped by concern.

*   **Purpose**: To group related logic into single, maintainable units.
*   **Examples**:
    *   `structure.toml`: Defines rules for file placement and directory layout.
    *   `git.toml`: Defines branch naming conventions and git workflow rules.
    *   `ast-analysis.toml`: Defines code-level checks (TODOs, debug prints).

### 2. Profiles: The "When" (Context Awareness)
**Location:** `.config/project-lint/rules/profiles/*.toml`

Profiles represent **Context-Aware Activations**. They define *under what conditions* a set of rules should be applied.

*   **Purpose**: To automatically detect the project type and apply relevant standards without manual user configuration.
*   **Mechanism**: Profiles use **Activation Triggers** to detect the environment:
    *   **Indicators**: Specific files (e.g., `package.json`, `Cargo.toml`).
    *   **Paths**: Specific directories (e.g., `src/web/`, `infra/`).
    *   **Globs**: File patterns (e.g., `**/*.tf`, `**/*.tsx`).
*   **Example**: The `web.toml` profile activates when it sees `package.json` or `.tsx` files, enabling HTML/CSS/JS checks defined in various slices.

### 3. Active Rules: The "Now" (Enforcement)
**Location:** `.config/project-lint/rules/active/*.toml`

These are the **Enforced Rules** for the current execution.

*   **Purpose**: This directory holds the specific rule sets that are currently "turned on".
*   **Function**: It allows for granular control where users (or the system) can drop a file here to enable a check without modifying a monolithic configuration file.

### 4. Core Configuration: The "Base" (Defaults & Overrides)
**Location:** `.config/project-lint/rules/core.toml` and `config.toml`

*   **`core.toml`**: The immutable system defaults shipped with the tool.
*   **`config.toml`**: The user's local overrides (Project, XDG, or Home). This layer always takes precedence, allowing users to say "I know this is a web project, but I want to disable the CSS check."

## Check Enablement Logic

The system supports a flexible mechanism for enabling and disabling specific checks using a **Allowlist/Denylist** model with profile merging.

### Modes
The `rules.mode` configuration determines how checks are evaluated:

*   **`denylist` (Default)**: All checks are enabled by default EXCEPT those explicitly disabled.
*   **`allowlist`**: All checks are disabled by default EXCEPT those explicitly enabled.

### Precedence and Merging
The effective set of enabled/disabled checks is calculated by merging configuration from the repository-level `config.toml` and all active **Profiles**.

1.  **Effective Enabled Checks** = Union of:
    *   Repo `rules.enabled_checks`
    *   All active profiles' `checks.enable` list

2.  **Effective Disabled Checks** = Union of:
    *   Repo `rules.disabled_checks`
    *   All active profiles' `checks.disable` list

### Evaluation
*   In **Allowlist** mode: A check is run ONLY if it appears in the **Effective Enabled Checks** list.
*   In **Denylist** mode: A check is run UNLESS it appears in the **Effective Disabled Checks** list.

## Hierarchy of Application

1.  **Profile Detection**: The system scans the project to see which **Profiles** match the current context (e.g., "This is a Web + DevOps project").
2.  **Slice Selection**: Active profiles select relevant **Slices** of rules to apply (e.g., "Enable HTML checks" and "Enable Docker checks").
3.  **User Override**: The system applies local **Config** overrides to fine-tune the final rule set.

## Directory Structure

```
.config/project-lint/
├── config.toml             # User/Project overrides
└── rules/
    ├── core.toml           # System defaults
    ├── active/             # Currently enforced rules
    ├── profiles/           # Context definitions (Web, DevOps, Rust, etc.)
    └── slices/             # Domain logic definitions (Git, AST, Structure)

## Rule Definition Structure

Each TOML rule file follows a structured schema with both standardized system sections and domain-specific sections.

### Standardized System Sections
These sections MUST be present in every slice file to ensure consistent processing and reporting.

1.  **`[metadata]`**: Provides identity and versioning for the slice.
    *   `name`: Unique identifier for the slice (e.g., `"git-slice"`).
    *   `version`: Semantic version of the rule set.
    *   `scope`: The domain this slice covers (e.g., `"git-operations"`).
    *   `updated`: Last modification date.
    *   `description`: Human-readable explanation of the slice's purpose.

2.  **`[messages]`**: Defines the user-facing output strings for rule violations.
    *   Allows for easy localization and customization of error messages without changing code.
    *   Supports placeholders like `{file}`, `{branch}`, `{line}`, etc.

### Slice-Specific Sections
These sections are unique to the domain of the slice and contain the actual configuration logic.

#### Example: Git Slice (`git.toml`)
*   `[git_branch]`: configuration for branch allow/deny lists.
*   `[branch_patterns]`: Regex patterns for enforcing naming conventions.
*   `[commit_messages]`: Rules for commit message formatting and length.
*   `[repository_state]`: Checks for clean working directory, large files, etc.

#### Example: Structure Slice (`structure.toml`)
*   `[directory_structure]`: Defines required and forbidden directories.
*   `[file_placement]`: Rules mapping specific file types to directories.
*   `[organization_patterns]`: Pre-defined patterns for specific project types (e.g., `rust_project`, `node_project`).

#### Example: AST Analysis Slice (`ast-analysis.toml`)
*   `[ast_analysis]`: Global settings for the AST engine (timeouts, file sizes).
*   `[supported_languages]`: Toggles for enabling analysis per language.
*   `[query_patterns]`: Tree-sitter queries used to detect issues.
*   `[severity_mapping]`: Maps specific findings to warning/error levels.

#### Example: Extensions Slice (`extensions.toml`)
*   `[file_types]`: Groups extensions into logical categories (e.g., `source_files`, `images`).
*   `[extension_mappings]`: specific mapping of extensions to target folders.
*   `[content_analysis]`: Toggles for magic-number/signature verification.

## Profile Activation Logic

### Activation Conditions: OR Logic

Profile activation uses **OR logic** across all conditions. This means a profile is activated if **any single condition is met**.

#### Between Categories
If you define multiple activation categories (e.g., `indicators`, `paths`, `globs`, `content`), the profile activates if **any** category matches:

```toml
[activation]
indicators = ["package.json"]       # OR
paths = ["src/web/"]               # OR
globs = ["**/*.tsx"]               # OR
[[activation.content]]
matches = ["React"]
globs = ["**/*.js"]
```

In this example, the profile activates if:
- `package.json` exists, **OR**
- `src/web/` directory exists, **OR**
- Any `.tsx` files exist, **OR**
- Any `.js` file contains the string "React"

#### Within Categories
If you define multiple items within a single category, the profile activates if **any** item matches:

```toml
[activation]
paths = ["src/", "lib/", "app/"]   # Activates if ANY of these exist
indicators = ["Cargo.toml", "setup.py", "package.json"]  # Activates if ANY exists
```

### Design Rationale

This OR-based approach implements a **discovery mechanism**:
- "If I see *any* evidence that this is a [Project Type], treat it as one."
- Profiles are designed to be inclusive rather than restrictive.
- A project can match multiple profiles (e.g., a monorepo with both web and DevOps components).

**Important**: Profile activation is a **rough decision**. Once a profile is activated, individual rules within the associated slices can further narrow down their applicability based on more specific conditions. For example:
- A profile might activate on any `.js` file (broad trigger).
- But a specific rule within that profile might only apply to `.js` files in the `src/` directory (narrower condition).
- This two-stage filtering prevents false positives while maintaining fast profile discovery.

### Use Cases

*   **Web Profile**: Activates on `package.json` OR `.tsx` files OR `vite.config.js`.
*   **DevOps Profile**: Activates on `Dockerfile` OR `terraform/` directory OR `*.tf` files.
*   **Rust Profile**: Activates on `Cargo.toml` OR `src/` directory containing `.rs` files.
```
