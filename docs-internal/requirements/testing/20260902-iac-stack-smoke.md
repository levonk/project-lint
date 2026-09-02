# Smoke Test: IaC Stack Scanners (terraform_lint, pulumi_lint, ansible_lint, jinja_template)

**Date**: 2026-09-02
**PRD**: [docs-internal/requirements/20260901-iac-stack.md](../20260901-iac-stack.md)
**Build**: `devbox run -- just build` (release)

## Objective

Verify that the 4 new IaC stack scanners are:
1. Silent on repos without matching files (no false positives)
2. Fire correctly when matching files with violations are present
3. Respect the centralized exclusion list

## Scanners Tested

| Scanner | Check name | File types |
|---------|-----------|------------|
| terraform_lint | `terraform_lint` | `.tf`, `.tfvars`, `.tf.json` |
| pulumi_lint | `pulumi_lint` | `Pulumi.yaml`, `Pulumi.*.yaml` |
| ansible_lint | `ansible_lint` | `ansible.cfg`, `*.yml` in `ansible/`, `playbooks/`, `roles/` |
| jinja_template | `jinja_template` | `.j2`, `.jinja2` |

## Results

### Repos without IaC files (silent — no false positives)

| Repo | .tf | Pulumi.yaml | ansible.cfg | .j2 | Result |
|------|-----|-------------|-------------|-----|--------|
| `project-lint` | 0 | 0 | 0 | 0 | ✅ Silent (no Terraform/Pulumi/Ansible/Jinja issues) |
| `devbox` | 0 | 0 | 0 | 0 | ✅ Silent |

### Repos with matching files (scanners fire)

| Repo | File | Scanner | Rule fired | Severity |
|------|------|---------|-----------|----------|
| `infrahub` | `ansible.cfg` | ansible_lint | `ansible-cfg-no-host-key-checking` | error |
| `mrepo` | `proj/deployment-operations/ansible.cfg` | ansible_lint | `ansible-cfg-no-host-key-checking` | error |
| `levonk-base-boilerplate` | `.devcontainer/*.j2` | jinja_template | Silent (clean templates — no secret literals, no eval/exec, no absolute paths) | — |

### Terraform (forward-looking — 0 .tf files in all repos)

No `.tf` files exist in any repo under `~/p/gh/levonk/`. The terraform_lint
scanner is silent on all repos. Verified via unit tests that it fires when
`.tf` files with violations are placed in a temp dir.

### Pulumi (forward-looking — 0 Pulumi.yaml files in all repos)

No `Pulumi.yaml` files exist in any repo under `~/p/gh/levonk/`. The
pulumi_lint scanner is silent on all repos. Verified via unit tests that it
fires when `Pulumi.yaml` with violations is placed in a temp dir.

### Jinja2 templates (levonk-base-boilerplate)

The `levonk-base-boilerplate` repo has 4 `.j2` files in `.devcontainer/`:
- `devcontainer.json.j2`
- `Dockerfile.build.j2`
- `Dockerfile.base.j2`
- `Dockerfile.run.j2`

The jinja_template scanner is **silent** on all of these — they are clean
templates that do not contain:
- Secret literal variable names (`secret_value`, `vault_password`, etc.)
- Forbidden filters (`eval`, `exec`)
- Absolute paths in `{% include %}` / `{% extends %}`
- Raw includes via `{{ include }}`

This is the correct behavior — the scanner fires only on actual violations,
not on every `.j2` file.

## Exclusion List Verification

All 4 scanners use `walk_project()` with `build_exclusions()` from the
centralized exclusion list. Verified that:
- `node_modules/`, `target/`, `dist/`, `build/`, `.git/` are skipped
- No IaC issues are reported from excluded directories

## Unit Test Coverage

| Scanner | Tests | Coverage |
|---------|-------|----------|
| terraform_lint | 8 | silent, hardcoded secret, variable desc/type, clean file, missing lockfile, provider version, missing backend, config disable, empty file |
| pulumi_lint | 7 | silent, missing name/runtime, secret in config, clean, config disable, empty file, Pulumi.dev.yaml variant |
| ansible_lint | 9 | silent, become at play, task missing name, host key checking, vault password file, clean playbook, config disable, empty playbook, ignores non-ansible yml |
| jinja_template | 8 | silent, secret literal, forbidden filter, absolute path, clean template, config disable, empty template, .jinja2 extension |

**Total**: 32 new unit tests, all passing.

## Conclusion

All 4 IaC stack scanners are working correctly:
- ✅ Silent on repos without matching files (no false positives)
- ✅ Fire correctly when matching files with violations are present
- ✅ Respect the centralized exclusion list
- ✅ Forward-looking scanners (terraform_lint, pulumi_lint) are silent today and ready for future use
- ✅ All 32 unit tests pass
- ✅ `devbox run -- just quality` passes (fmt + clippy + tests)
