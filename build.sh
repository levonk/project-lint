#!/bin/bash

echo "🔧 Building project-lint..."

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found. Are you in the project root?"
    exit 1
fi

# Check project structure
echo "📁 Checking project structure..."
required_files=(
    "src/main.rs"
    "src/lib.rs"
    "src/config.rs"
    "src/git.rs"
    "src/utils.rs"
    "src/commands/mod.rs"
    "src/commands/init.rs"
    "src/commands/lint.rs"
    "src/commands/watch.rs"
    "README.md"
    "docs-internal/requirements/20250804initial-project-lint-requirements.md"
)

for file in "${required_files[@]}"; do
    if [ -f "$file" ]; then
        echo "✅ $file"
    else
        echo "❌ Missing: $file"
        exit 1
    fi
done

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Cargo not found. Please install Rust."
    exit 1
fi

echo "✅ Project structure looks good!"
echo "🚀 Ready to build with: cargo build" 