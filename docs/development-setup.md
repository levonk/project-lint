# Development Environment Setup

This guide covers setting up the development environment for project-lint using Devbox.

## Prerequisites

- [Devbox](https://www.jetpack.io/devbox/docs/installing-devbox/) installed on your system

## Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/project-lint.git
cd project-lint

# Initialize the development environment
devbox shell

# Build the project
cargo build

# Run tests
cargo test
```

## Available Scripts

Once inside the Devbox shell, you have access to the following scripts:

```bash
# Build the project
devbox run build

# Run tests
devbox run test

# Run linter
devbox run lint

# Format code
devbox run format

# Run the application
devbox run run

# Run in watch mode (auto-rebuild on changes)
devbox run dev
```

## Environment Variables

The Devbox environment sets up the following environment variables:

- `RUST_LOG=debug`: Enables debug logging
- `RUST_BACKTRACE=1`: Enables backtrace for errors

## Development Tools

The Devbox environment includes:

- **Rust toolchain**: Latest stable Rust with rust-analyzer, clippy, and rustfmt
- **Git**: Version control system
- **ctags**: For code navigation

## Manual Setup (Alternative)

If you prefer not to use Devbox, you can manually install:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install additional tools
cargo install cargo-watch
```

## Troubleshooting

### Issues with Devbox

```bash
# Clear Devbox cache
devbox clean

# Rebuild the environment
devbox shell --rebuild
```

### Rust Toolchain Issues

```bash
# Update Rust toolchain
rustup update

# Install missing components
rustup component add rust-analyzer clippy rustfmt
```

## Next Steps

After setting up the environment:

1. Read the [Architecture Guide](architecture.md)
2. Check out the [TUI Configuration Guide](tui-configuration.md)
3. Learn about [Hook Installation](hook-installation.md)
4. Understand [Hook Logging](hook-logging.md)
