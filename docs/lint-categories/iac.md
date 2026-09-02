# Infrastructure-as-Code Rules

IaC rules validate Terraform, Pulumi, Ansible, and Jinja2 template files for
security, best practices, and maintainability.

## Overview

IaC rules help identify:
- Hardcoded secrets in infrastructure definitions
- Missing required fields in configuration files
- Insecure defaults (host key checking, vault passwords)
- Missing descriptions and type annotations
- Dangerous template filters and absolute paths

## Scanners

### terraform_lint

Validates `.tf` / `.tfvars` / `.tf.json` files.

**Check name**: `terraform_lint`

**Configuration** (`[scanner_config.terraform_lint]`):
```toml
[scanner_config.terraform_lint]
require_backend = true
require_lockfile = true
forbid_hardcoded_secrets = true
```

**Rules**:
- `tf-no-hardcoded-secrets` (error) — hardcoded secret literals (password, api_key, etc.)
- `tf-variable-description` (info) — variable blocks should have a description
- `tf-variable-type` (warning) — variable blocks should have a type field
- `tf-output-description` (info) — output blocks should have a description
- `tf-provider-version` (warning) — provider blocks should pin version
- `tf-resource-naming` (warning) — resource names should be descriptive (not foo/bar/test)
- `tf-no-default-tags-in-resource` (info) — use default_tags in provider, not per-resource tags
- `tf-backend-config` (warning) — terraform block should define a backend (project-level)
- `tf-lockfile-present` (warning) — .terraform.lock.hcl should be committed (project-level)

### pulumi_lint

Validates `Pulumi.yaml` / `Pulumi.*.yaml` files.

**Check name**: `pulumi_lint`

**Configuration** (`[scanner_config.pulumi_lint]`):
```toml
[scanner_config.pulumi_lint]
require_config = true
forbid_secrets_in_config = true
```

**Rules**:
- `pulumi-name-present` (error) — Pulumi.yaml must have a name field
- `pulumi-runtime-set` (error) — Pulumi.yaml must have a runtime field
- `pulumi-config-present` (info) — Pulumi.yaml should have a config section
- `pulumi-no-secrets-in-config` (error) — no plaintext secrets in config
- `pulumi-description-present` (info) — Pulumi.yaml should have a description

### ansible_lint

Validates Ansible playbooks (in `ansible/`, `playbooks/`, `roles/` directories)
and `ansible.cfg` files.

**Check name**: `ansible_lint`

**Configuration** (`[scanner_config.ansible_lint]`):
```toml
[scanner_config.ansible_lint]
forbid_host_key_checking = true
require_task_names = true
```

**Rules**:
- `ansible-no-become-true-at-play` (warning) — become: true should be at task level
- `ansible-no-hardcoded-vault-password` (error) — no literal vault password paths
- `ansible-task-name-present` (warning) — tasks should have a name field
- `ansible-no-command-shell` (info) — avoid command/shell modules when dedicated modules exist
- `ansible-cfg-no-host-key-checking` (error) — ansible.cfg must not disable host key checking
- `ansible-cfg-vault-password-file` (error) — ansible.cfg must not commit vault password file path

### jinja_template

Validates `.j2` / `.jinja2` template files.

**Check name**: `jinja_template`

**Configuration** (`[scanner_config.jinja_template]`):
```toml
[scanner_config.jinja_template]
forbidden_filters = ["eval", "exec"]
forbid_absolute_paths = true
```

**Rules**:
- `jinja-no-secret-literals` (warning) — templates should not reference secret variable names
- `jinja-sandbox-filters` (error) — templates must not use eval/exec filters
- `jinja-no-absolute-paths` (warning) — include/extends paths should be relative
- `jinja-no-raw-include` (info) — prefer {% include %} / {% extends %} directives

## Forward-Looking Design

These scanners are designed to be forward-looking. Most repos today do not
contain `.tf`, `Pulumi.yaml`, or `.j2` files. The scanners are:

1. **Silent when no matching files exist** — no false positives on non-IaC repos
2. **Gated by check names** — can be enabled/disabled via config
3. **Ready for future use** — Pulumi is on the project roadmap

## Examples

### Terraform

❌ **Bad:**
```hcl
resource "aws_instance" "foo" {
  password = "hunter2"
}
```

✅ **Good:**
```hcl
variable "db_password" {
  description = "Database password"
  type        = string
  sensitive   = true
}

resource "aws_instance" "web" {
  password = var.db_password
}
```

### Pulumi

❌ **Bad:**
```yaml
config:
  db_password: "hunter2"
```

✅ **Good:**
```yaml
name: my-project
runtime: nodejs
description: My project
config:
  region: us-east-1
```

### Ansible

❌ **Bad:**
```yaml
- hosts: all
  become: true
  tasks:
    - apt: name=nginx
```

✅ **Good:**
```yaml
- hosts: all
  tasks:
    - name: Install nginx
      apt: name=nginx
```

### Jinja2

❌ **Bad:**
```jinja
{{ code | eval }}
{% include '/etc/nginx.conf' %}
```

✅ **Good:**
```jinja
{% extends 'base.html' %}
{% block content %}Hello {{ name }}{% endblock %}
```
