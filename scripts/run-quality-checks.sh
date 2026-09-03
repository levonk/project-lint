#!/usr/bin/env bash
# Shared quality-checks script for project-lint.
#
# Usage:
#   scripts/run-quality-checks.sh           # FAST_MODE (pre-commit): fmt check + clippy + tests
#   scripts/run-quality-checks.sh --full    # FULL_MODE (CI): + bench + doc tests + audit
#
# Exits non-zero on the first failing stage so it can be used as a CI gate
# and a pre-commit hook. Designed for parity with the `just quality` recipe.

set -euo pipefail

# Re-exec through devbox when invoked from a bare shell (e.g. a git hook)
# so rustfmt/clippy/cargo resolve to the project toolchain. Inside devbox,
# DEVBOX_SHELL_ENABLED=1 prevents recursion. In CI, everything is on PATH
# already and devbox may be absent, so the guard is skipped.
if [[ "${DEVBOX_SHELL_ENABLED:-0}" != "1" ]] && command -v devbox >/dev/null 2>&1; then
    exec devbox run -- "$0" "$@"
fi

MODE="fast"
if [[ "${1:-}" == "--full" || "${CI:-}" == "true" ]]; then
    MODE="full"
fi

echo "▶ project-lint quality checks (${MODE})"

run() {
    echo "❯ $*"
    "$@"
}

# Stage 1 — formatting check (fast & full)
# Uses rustfmt directly with --edition 2021 because cargo fmt has a pre-existing
# issue resolving the edition for module parsing in this workspace.
# Avoid `find -not` (rtk find doesn't support it); use fd if available, else git ls-files.
if command -v fd >/dev/null 2>&1; then
    RUST_FILES=$(fd -e rs . --exclude target --exclude .git)
elif command -v git >/dev/null 2>&1; then
    RUST_FILES=$(git ls-files '*.rs' ':!:target/*' ':!:.git/*')
else
    RUST_FILES=$(ls -1 src/**/*.rs project-lint-core/src/**/*.rs tests/*.rs 2>/dev/null)
fi
# Resolve rustfmt — it is not always on the bare PATH (devbox keeps it inside
# the nix profile). Prefer PATH, then the devbox profile, then cargo fmt.
RUSTFMT_BIN=""
if command -v rustfmt >/dev/null 2>&1; then
    RUSTFMT_BIN=$(command -v rustfmt)
elif [[ -x .devbox/nix/profile/default/bin/rustfmt ]]; then
    RUSTFMT_BIN=.devbox/nix/profile/default/bin/rustfmt
fi
if [[ -n "$RUSTFMT_BIN" ]]; then
    run "$RUSTFMT_BIN" --edition 2021 --check $RUST_FILES
else
    run cargo fmt --all -- --check
fi

# Stage 2 — clippy (fast & full). Pre-existing warnings are allowed; only
# errors fail the gate. Use -D warnings once the pre-existing debt is cleared.
run cargo clippy --workspace --all-targets

# Stage 3 — workspace tests (fast & full)
run cargo test --workspace

# Stage 3b — frontmatter unquoted-colon check on staged .md files (pre-commit)
# Scans staged markdown files for unquoted colon-space sequences in YAML
# frontmatter values — the same bug class as the Rust scanners
# (skill-frontmatter-unquoted-colon / md-frontmatter-unquoted-colon).
# Only runs when there are staged .md files with frontmatter.
if command -v git >/dev/null 2>&1; then
    frontmatter_failures=0
    staged_md_files=""
    while IFS= read -r line; do
        case "$line" in
            node_modules/*|target/*) continue ;;
        esac
        if [[ -n "$line" ]]; then
            staged_md_files="${staged_md_files}${line}
"
        fi
    done < <(git diff --cached --name-only -- '*.md')
    if [[ -n "$staged_md_files" ]]; then
        while IFS= read -r f; do
            [[ -z "$f" ]] && continue
            [[ -f "$f" ]] || continue
            head -c 3 -- "$f" 2>/dev/null | grep -q '^---$' || continue
            awk '
                NR == 1 && $0 == "---" { in_fm = 1; next }
                in_fm && $0 == "---" { exit }
                in_fm {
                    if ($0 ~ /^[[:space:]]/) next
                    idx = index($0, ":")
                    if (idx == 0) next
                    key = substr($0, 1, idx - 1)
                    val = substr($0, idx + 1)
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", val)
                    if (val == "") next
                    if (substr(val, 1, 1) == "\"" || substr(val, 1, 1) == "'\''") next
                    if (index(val, ": ") > 0) {
                        printf "  %s: unquoted colon-space in field \"%s\": %s\n", FILENAME, key, val
                        found = 1
                    }
                }
                END { exit (found ? 1 : 0) }
            ' "$f" || frontmatter_failures=$((frontmatter_failures + 1))
        done <<< "$staged_md_files"
    fi
    if [[ "$frontmatter_failures" -gt 0 ]]; then
        echo "✗ frontmatter unquoted-colon check failed on ${frontmatter_failures} file(s)"
        echo "  Quote the value, e.g. description: \"For skills: create...\""
        exit 1
    fi
fi

if [[ "${MODE}" == "full" ]]; then
    # Stage 4 — doc tests
    run cargo test --workspace --doc

    # Stage 5 — benchmarks compile-check (does not run iterations)
    run cargo bench --workspace --no-run

    # Stage 6 — cargo audit (optional; skip if not installed)
    if command -v cargo-audit >/dev/null 2>&1; then
        run cargo audit --ignore RUSTSEC-0000-0000 || \
            echo "⚠️  cargo audit reported advisories (non-fatal)"
    else
        echo "ℹ️  cargo-audit not installed; skipping audit stage"
    fi
fi

echo "✓ quality checks passed (${MODE})"
