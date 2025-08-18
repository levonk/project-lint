# Project Lint - Initial Requirements

**Date**: 2025-08-04  
**Version**: 1.0  
**Status**: Initial Requirements

## Overview

Project-lint is a CLI tool designed to help developers maintain clean project structure by providing warnings and automated organization for file placement, git branch management, and project standards compliance.

## Core Requirements

### 1. Git Branch Validation
- **Requirement**: Warn users when creating files on inappropriate git branches
- **Implementation**: 
  - Detect current git branch
  - Check against configured allowed/forbidden branches
  - Provide clear warnings with branch context
- **Configuration**: TOML-based branch rules in config file

### 2. File Location Validation
- **Requirement**: Warn about files created in wrong directories
- **Implementation**:
  - Scan project structure for misplaced files
  - Check file types against directory purpose
  - Suggest correct locations based on file type
- **Examples**:
  - Scripts in root instead of `bin/` or `scripts/`
  - Source files in wrong directories
  - Documentation in incorrect locations

### 3. Automatic File Organization
- **Requirement**: Auto-move files based on type and project standards
- **Implementation**:
  - Detect file types (extensions, content analysis)
  - Move files to appropriate directories
  - Maintain git history during moves
  - Provide dry-run mode for safety

### 4. Script Directory Compliance
- **Requirement**: Ensure scripts are in the correct directory structure
- **Implementation**:
  - Detect when `scripts/` directory is created but project uses `bin/`
  - Warn about script placement inconsistencies
  - Suggest proper directory structure
- **Configuration**: Configurable preferred script directories

### 5. Configuration Management
- **Requirement**: TOML-based configuration with hierarchical fallback
- **Implementation**:
  - Project-specific: `.config/project-lint/config.toml`
  - XDG Config: `$XDG_CONFIG_HOME/project-lint/config.toml`
  - Fallback: `~/.config/project-lint/config.toml`
- **Features**:
  - Default configurations for common project types
  - Custom rule definitions
  - File type mappings
  - Ignored patterns

## Technical Requirements

### Architecture
- **Language**: Rust
- **CLI Framework**: clap with derive macros
- **Configuration**: TOML with serde
- **Git Integration**: git2 crate
- **File Watching**: notify crate
- **Error Handling**: anyhow + thiserror
- **Logging**: tracing + tracing-subscriber

### Project Structure
```
project-lint/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── commands/            # Command implementations
│   │   ├── mod.rs
│   │   ├── init.rs          # Initialize configuration
│   │   ├── lint.rs          # Run linting checks
│   │   └── watch.rs         # File watching
│   ├── config.rs            # Configuration management
│   ├── git.rs               # Git operations
│   └── utils.rs             # Utilities and error handling
├── tests/                   # Test files
├── examples/                # Example configurations
├── docs-internal/           # Internal documentation
│   └── requirements/        # Requirements documents
└── Cargo.toml              # Dependencies and metadata
```

### CLI Commands
1. **`init`**: Initialize project-lint configuration
   - Create default config files
   - Set up project-specific settings
   - Force overwrite option

2. **`lint`**: Run linting checks
   - Git branch validation
   - File location checks
   - Directory structure validation
   - Custom rule execution

3. **`watch`**: Monitor file changes
   - Real-time file system monitoring
   - Automatic linting on changes
   - Debounced event handling

### Configuration Schema
```toml
[git]
warn_wrong_branch = true
allowed_branches = ["main", "master", "feature/*"]
forbidden_branches = ["develop", "staging"]

[files]
auto_move = true
type_mappings = { "*.sh" = "bin/", "*.py" = "scripts/" }
ignored_patterns = ["node_modules/", ".git/"]

[directories]
warn_scripts_location = true
scripts_directory = "bin"
structure = { "src/" = ["*.rs", "*.py"] }

[rules]
enabled_checks = ["git_branch", "file_location"]
custom_rules = [
    { name = "no_tmp", pattern = "*.tmp", message = "No temp files", severity = "error" }
]
```

## Non-Functional Requirements

### Performance
- Fast file system scanning
- Efficient git operations
- Minimal memory footprint
- Responsive file watching

### Usability
- Clear, colored output
- Helpful error messages
- Intuitive command structure
- Comprehensive help documentation

### Reliability
- Graceful error handling
- Safe file operations
- Backup mechanisms for file moves
- Configurable verbosity levels

### Extensibility
- Plugin architecture for custom rules
- Configurable file type detection
- Custom validation hooks
- Integration with existing tools

## Future Enhancements

### Phase 2 Features
- IDE/Editor integration
- Pre-commit hooks
- CI/CD pipeline integration
- Project templates
- Multi-language support

### Phase 3 Features
- Machine learning for file classification
- Team collaboration features
- Advanced git workflow integration
- Performance analytics

## Success Criteria

1. **Adoption**: Easy setup and immediate value
2. **Accuracy**: Reliable detection of issues
3. **Performance**: Fast execution on large projects
4. **Usability**: Intuitive interface and helpful output
5. **Extensibility**: Easy to customize for different project types

## Risk Mitigation

### Technical Risks
- **Git Integration Complexity**: Use mature git2 crate
- **File System Performance**: Implement efficient scanning algorithms
- **Cross-Platform Compatibility**: Test on multiple platforms

### User Experience Risks
- **False Positives**: Configurable sensitivity levels
- **Learning Curve**: Comprehensive documentation and examples
- **Integration Issues**: Clear migration paths and compatibility

## Timeline

- **Week 1-2**: Core architecture and basic CLI
- **Week 3-4**: Git integration and file validation
- **Week 5-6**: Configuration system and file watching
- **Week 7-8**: Testing, documentation, and refinement
- **Week 9-10**: Beta testing and bug fixes
- **Week 11-12**: Release preparation and documentation 