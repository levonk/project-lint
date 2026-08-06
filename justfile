# project-lint - Modular, multi-faceted linting and organization tool
# Standard justfile following ADR-20260131001

# Normal targets - Developer interface (REQUIRED)
clean:
    devbox run clean

dev:
    devbox run dev

build:
    devbox run build

test:
    devbox run test

lint:
    devbox run lint

typecheck:
    devbox run typecheck

release:
    devbox run release

# Bootstrap recipes (REQUIRED)
bootstrap:
    devbox run bootstrap

bootstrap-internal:
    # Internal bootstrap logic called by devbox init_hook
    # Language-specific dependency installation
    just setup
    echo "✅ Project bootstrap complete!"

# Health and diagnostics (REQUIRED)
doctor:
    devbox run doctor

doctor-internal:
    # Check development environment health
    echo "🔍 Checking project-lint development environment..."
    @if ! cargo --version >/dev/null 2>&1; then \
        echo "❌ Error: cargo not found" >&2; \
        echo "💡 Suggestion: Ensure Rust toolchain is installed" >&2; \
        exit 1; \
    fi
    @if ! just --version >/dev/null 2>&1; then \
        echo "❌ Error: just not found" >&2; \
        echo "💡 Suggestion: Ensure just is installed" >&2; \
        exit 1; \
    fi
    @if [ ! -f Cargo.toml ]; then \
        echo "❌ Error: Cargo.toml not found (expected in project root)" >&2; \
        exit 1; \
    fi
    echo "✅ OK: Rust toolchain + just + Cargo.toml present"
    @if command -v direnv >/dev/null 2>&1; then \
        echo "✅ OK: direnv present"; \
        echo "💡 Next: direnv allow"; \
    else \
        echo "⚠️  Warning: direnv not found"; \
        echo "💡 Suggestion: install direnv (https://direnv.net/)"; \
        echo "💡 Then run: direnv allow"; \
    fi
    echo "💡 Suggestion: just bootstrap"
    echo "🚀 Ready to develop project-lint!"

# Quality checks (OPTIONAL but RECOMMENDED)
quality:
    scripts/run-quality-checks.sh

# Full quality checks (CI parity): fmt + clippy + tests + doc tests + bench compile + audit
quality-full:
    scripts/run-quality-checks.sh --full

# Formatting (REQUIRED standard target)
fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Benchmarks (compile-check only by default; `just bench-run` to execute)
bench:
    cargo bench --workspace --no-run

bench-run:
    cargo bench --workspace

# CI entry point — used by .github/workflows/ci.yml
ci:
    scripts/run-quality-checks.sh --full

# Language-specific commands for Rust CLI
# Development setup (OPTIONAL)
setup:
    echo "🦀 Rust CLI development environment ready!"

# Internal targets - Actual implementation
clean-internal:
    # Clean build artifacts
    cargo clean
    echo "🧹 Build artifacts removed"

build-internal:
    # Build the project in release mode
    cargo build --release

release-internal:
    # Full release pipeline: quality checks + build
    echo "🚀 Starting release pipeline for project-lint..."
    just lint-internal
    just test-internal
    just typecheck-internal
    just build-internal
    echo "✅ Release complete! Binary available at target/release/project-lint"

debug-internal:
    # Build the project in debug mode
    cargo build

install-internal:
    # Install the binary locally
    cargo install --path .

lint-internal:
    # Lint the code using clippy
    cargo clippy -- -D warnings

test-internal:
    # Run tests
    cargo test

typecheck-internal:
    # Run type checking (cargo check)
    cargo check

dev-internal:
    # Run the application in development mode
    cargo run

run-internal:
    # Run the application with arguments
    cargo run

# Help target
help:
    # Show available commands
    echo "🔧 project-lint - Rust Linting and Organization Tool"
    echo ""
    echo "Standard commands:"
    echo "  just bootstrap    - Initialize the development environment"
    echo "  just build        - Build the project"
    echo "  just test         - Run tests"
    echo "  just lint         - Run linting"
    echo "  just typecheck    - Run type checking"
    echo "  just dev           - Run in development mode"
    echo "  just clean         - Clean build artifacts"
    echo "  just doctor        - Check environment health"
    echo "  just quality       - Run all quality checks"
    echo "  just release       - Full release pipeline"
    echo ""
    echo "Rust-specific commands:"
    echo "  just debug         - Build in debug mode"
    echo "  just install       - Install binary locally"
    echo ""
    echo "Internal commands (for devbox scripts):"
    echo "  just *-internal    - Internal implementations"
