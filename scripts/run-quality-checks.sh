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
