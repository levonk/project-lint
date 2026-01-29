# Git Hooks and CI/CD Integration

Project-lint can integrate with Git hooks and CI/CD platforms to enforce code quality throughout the development lifecycle.

## Git Hooks Integration

### Installation

```bash
# Install git hooks
project-lint install-hook --agent git-hooks

# Install to custom directory
project-lint install-hook --agent git-hooks --dir /path/to/repo/.git
```

### Available Git Hooks

#### Pre-commit Hook
- **Trigger**: Before each commit
- **Purpose**: Check staged files for lint issues
- **Behavior**: Blocks commit if issues found
- **Fix**: Run `project-lint lint --fix` to resolve

#### Pre-push Hook
- **Trigger**: Before each push
- **Purpose**: Full repository lint check
- **Behavior**: Blocks push if issues found
- **Fix**: Run `project-lint lint --fix` to resolve

### Hook Behavior

```bash
# When you try to commit
git commit -m "Add new feature"

# Output:
# Running project-lint pre-commit checks...
# ❌ project-lint found issues. Please fix them before committing.
# Run 'project-lint lint --fix' to auto-fix issues.
```

### Manual Hook Management

```bash
# Enable hooks
git config core.hooksPath .git/hooks

# Disable hooks temporarily
git config core.hooksPath /dev/null

# Skip hooks for one commit
git commit --no-verify -m "Commit message"
```

## GitHub Actions Integration

### Installation

```bash
# Install GitHub workflows
project-lint install-hook --agent github
```

### Created Workflows

#### 1. Main Workflow (`.github/workflows/project-lint.yml`)
- **Triggers**: Push to main/develop, Pull Requests
- **Jobs**:
  - `project-lint`: Full lint check
  - `security-scan`: Security vulnerability scan
- **Features**:
  - Rust caching for faster builds
  - Artifact upload for failed runs
  - Security issue detection

#### 2. PR Workflow (`.github/workflows/project-lint-pr.yml`)
- **Triggers**: Pull Request events
- **Features**:
  - Changed files analysis
  - PR comments on failures
  - Focused linting on changes

### Workflow Features

```yaml
# Caching for faster builds
- name: Cache cargo registry
  uses: actions/cache@v3
  with:
    path: ~/.cargo/registry
    key: ${{ runner.os }}-cargo-registry-${{ hash('**/Cargo.lock') }}

# Artifact upload on failure
- name: Upload lint results
  if: failure()
  uses: actions/upload-artifact@v3
  with:
    name: lint-results
    path: |
      project-lint.log
      .local/share/project-lint/logs/
```

### Customization

Edit `.github/workflows/project-lint.yml` to:
- Change trigger branches
- Add more jobs (test, build, deploy)
- Modify caching strategy
- Add notifications

## GitLab CI Integration

### Installation

```bash
# Install GitLab CI configuration
project-lint install-hook --agent gitlab
```

### Created Files

#### 1. CI Configuration (`.gitlab-ci.yml`)
- **Stages**: lint → security → deploy
- **Jobs**:
  - `lint`: Main lint check
  - `security-scan`: Security analysis
  - `lint-merge-request`: MR-specific checks
  - `scheduled-security-scan`: Periodic security scans

#### 2. MR Template (`.gitlab/merge_request_templates/project-lint.md`)
- Template for merge requests
- Includes project-lint checklist
- Standardized MR format

### GitLab CI Features

```yaml
# Multi-stage pipeline
stages:
  - lint
  - security
  - deploy

# Caching for performance
cache:
  key: $CI_COMMIT_REF_SLUG
  paths:
    - .cargo/
    - target/

# MR-specific rules
lint-merge-request:
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

### Advanced Features

#### Scheduled Scans
```yaml
scheduled-security-scan:
  rules:
    - if: $CI_PIPELINE_SOURCE == "schedule"
  script:
    - ./target/release/project-lint lint --fix --dry-run
```

#### Security Reporting
```yaml
security-scan:
  artifacts:
    reports:
      security: security-report.json
```

## Platform Comparison

| Feature | Git Hooks | GitHub Actions | GitLab CI |
|---------|-----------|----------------|-----------|
| **Local Development** | ✅ | ✅* | ❌ |
| **Pre-commit Checks** | ✅ | ✅ | ✅ |
| **PR Integration** | ❌ | ✅ | ✅ |
| **Scheduled Scans** | ❌ | ✅ | ✅ |
| **Artifact Storage** | ❌ | ✅ | ✅ |
| **Security Reports** | ❌ | ✅ | ✅ |
| **Manual Override** | ✅ | ✅ | ✅ |

* Via the https://github.com/nekos/ACT tool

## Configuration Options

### Git Hooks Configuration
```bash
# Custom hook directory
project-lint install-hook --agent git-hooks --dir /custom/.git

# Force overwrite
project-lint install-hook --agent git-hooks --force
```

### GitHub Actions Customization
```yaml
# Custom branches
on:
  push:
    branches: [ main, develop, staging ]

# Custom Rust version
- name: Setup Rust
  uses: dtolnay/rust-toolchain@1.70.0
```

### GitLab CI Customization
```yaml
# Custom Docker image
lint:
  image: rust:1.70.0

# Custom variables
variables:
  PROJECT_LINT_ARGS: "--fix --dry-run"
```

## Best Practices

### 1. Local Development
- Use git hooks for immediate feedback
- Run `project-lint lint --fix` before committing
- Configure editor integration for real-time checks

### 2. CI/CD Integration
- Use both local hooks and CI checks
- Configure appropriate caching
- Set up notifications for failures

### 3. Security
- Run security scans in CI/CD
- Use scheduled scans for periodic checks
- Review security reports regularly

### 4. Performance
- Cache dependencies in CI/CD
- Use incremental linting where possible
- Optimize rule configurations

## Troubleshooting

### Git Hooks Issues
```bash
# Check hook permissions
ls -la .git/hooks/

# Test hook manually
./.git/hooks/pre-commit

# Reinstall hooks
project-lint install-hook --agent git-hooks --force
```

### GitHub Actions Issues
```yaml
# Debug workflow
- name: Debug info
  run: |
    echo "Current directory: $(pwd)"
    echo "Files: $(ls -la)"
    ./target/release/project-lint --version
```

### GitLab CI Issues
```yaml
# Debug GitLab CI
debug:
  stage: lint
  script:
    - echo "CI_PROJECT_DIR: $CI_PROJECT_DIR"
    - echo "Files: $(ls -la)"
    - ./target/release/project-lint --version
```

## Migration Guide

### From Pre-commit
```bash
# Remove pre-commit hooks
pre-commit uninstall

# Install project-lint hooks
project-lint install-hook --agent git-hooks

# Update .pre-commit-config.yaml rules to project-lint config
```

### From ESLint
```yaml
# Add to GitHub Actions
- name: Run project-lint
  run: |
    project-lint lint --fix --dry-run
  # Replace ESLint step
```

## Examples

### Complete Setup
```bash
# 1. Install git hooks
project-lint install-hook --agent git-hooks

# 2. Install GitHub workflows
project-lint install-hook --agent github

# 3. Configure local development
echo "export RUST_LOG=debug" >> ~/.bashrc

# 4. Test setup
git commit --allow-empty -m "test hooks"
```

### Custom Configuration
```bash
# Custom git hooks directory
project-lint install-hook --agent git-hooks --dir .custom-git/hooks

# Custom GitHub workflow
# Edit .github/workflows/project-lint.yml
# Add custom jobs or modify existing ones
```

This integration provides comprehensive code quality enforcement across the entire development lifecycle, from local development to production deployment.
