# Tilde-in-PATH Rules

The `tilde_path` scanner detects `~` used in `PATH` (and `*PATH`-style
search-path variables such as `MANPATH`, `PYTHONPATH`, `LD_LIBRARY_PATH`)
where the tilde will never expand into the user's home directory.

Tilde expansion only happens for *unquoted* tildes, and in assignments only
immediately after `=` or `:`. A quoted tilde in
`export PATH="$PATH:~/.local/bin"` stays literal — the shell searches
`./~/.local/bin` relative to the current directory, which both breaks the
lookup and lets a writable `~` directory shadow executables. File formats
with no tilde expansion at all (`Dockerfile` `ENV`, `.env`, YAML `PATH:`
keys, justfile/Makefile exports) have the same problem.

Reference: https://disconnect3d.pl//2026/10/02/dont-put-tilde-in-your-path/

## Configuration
```toml
[scanner_config.tilde_path]
check_quoted_tilde = true   # quoted/escaped ~ in PATH assignments (error)
check_literal_tilde = true  # ~ in literal formats: ENV, .env, YAML keys (error)
check_fragile_tilde = true  # unquoted ~ in shell assignments (info)
```

The scanner is gated by the `tilde_path` check name. Disable it via:
```toml
[rules]
disabled_checks = ["tilde_path"]
```

## Scanned files
- Shell scripts: `*.sh`, `*.bash`, `*.zsh`, `*.fish`, `*.ksh`, plus
  extensionless files with a shell shebang
- rc/profile files: `.bashrc`, `.zshrc`, `.profile`, `.envrc`, and friends
- Template variants of all of the above: `*.tmpl`, `*.template`, `*.j2`,
  `*.gotmpl`, and chezmoi names (`dot_zshrc`, `private_dot_bashrc.tmpl`,
  `run_once_*.sh.tmpl`)
- `*.nix` (e.g. `shellHook` strings), `*.yml`/`*.yaml` (env vars and
  workflow `run:` blocks)
- Literal formats: `Dockerfile`/`Containerfile`, `.env`/`*.env`,
  justfile/`*.just`, `Makefile`/`*.mk` (recipe lines are treated as shell)

## Rules
- `no-quoted-tilde-in-path` (error) — `~` inside quotes or backslash-escaped
  in a PATH assignment never expands
- `no-literal-tilde-in-path` (error) — `~` in a PATH value where the file
  format performs no tilde expansion
- `fragile-tilde-in-path` (info) — unquoted `~` in a shell assignment;
  expands via bash/zsh `:`/`=` rules but breaks on whitespace and in
  helper calls like `PATH_add`

## Examples
✅ Good:
```bash
export PATH="$PATH:$HOME/.local/bin"
PATH_add "$HOME/bin"
```
```dockerfile
ENV PATH=/usr/local/bin:/home/app/.local/bin
```

❌ Bad:
```bash
export PATH="$PATH:~/.local/bin"   # ~ never expands inside quotes
PATH_add "~/bin"                   # argument, not an assignment
```
```dockerfile
ENV PATH=/usr/local/bin:~/.local/bin   # ENV performs no tilde expansion
```
```yaml
environment:
  PATH: "~/bin:/usr/bin"           # literal tilde inside the container
```

## Out of Scope
- Runtime detection of `~` already in `$PATH` — check your shell with
  `echo "$PATH" | tr ':' '\n' | grep '~'`.
