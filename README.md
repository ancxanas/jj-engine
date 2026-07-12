# AVCS - Autonomous Version Control System

AVCS is an intelligent version control assistant that works with [Git](https://git-scm.com/) to automatically analyze, classify, and cluster code changes. It identifies the intent behind modifications and generates appropriate commit messages with smart auto-commit policies.

## Features

- **Intent Classification** - Automatically detects change types: Features, Bugfixes, Refactors, Test Additions, Documentation Updates, Dependency Updates, Configuration Changes, and Security Updates
- **Semantic Analysis** - Uses tree-sitter for deep code understanding and structural change detection
- **Intelligent Clustering** - Groups related files into coherent commits based on dependencies and relationships
- **Auto-Commit Policy** - Evaluates changes and determines which are safe to auto-commit vs. require review
- **Evidence Detection** - Identifies change intent from code patterns (test files, security checks, API changes, etc.)
- **Flexible Output** - Supports both human-readable and JSON output formats

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/avcs`.

## Quick Start

```bash
# Initialize AVCS in your project
avcs init

# Analyze working copy changes
avcs analyze

# Preview proposed commits
avcs preview

# Auto-commit safe changes
avcs commit --auto

# Show current status
avcs status
```

## Commands

| Command | Description |
| --------- | ------------- |
| `avcs init` | Initialize AVCS in the current project |
| `avcs analyze` | Analyze working copy changes and show intents |
| `avcs preview` | Preview proposed commits before committing |
| `avcs commit [--auto]` | Commit changes; use `--auto` for safe intents only |
| `avcs explain <id>` | Explain a specific change by its ID |
| `avcs status` | Show current status of working copy |

### Global Options

- `--json` - Output results as JSON instead of human-readable format

## How It Works

AVCS operates in several stages:

1. **Diff Collection** - Retrieves working copy changes from git
2. **Semantic Parsing** - Uses tree-sitter to parse TypeScript/JavaScript files and extract structural changes
3. **Evidence Detection** - Analyzes code for patterns indicating change intent (test additions, security patterns, refactoring indicators)
4. **Classification** - Matches changes against rules to determine the primary intent (feature, bugfix, refactor, etc.)
5. **Clustering** - Builds a relationship graph to group related files into commits
6. **Policy Evaluation** - Applies policy rules to determine which changes can be auto-committed

### Change Patterns

AVCS classifies changes into these patterns:

- **Feature** - New functionality additions
- **Bugfix** - Bug corrections
- **Refactor** - Code restructuring without behavior change
- **TestAddition** - New or modified test files
- **Documentation** - Docs-only changes
- **DependencyUpdate** - Package manager updates
- **Configuration** - Project configuration changes
- **Security** - Security-related modifications
- **Unknown** - Unclassified changes

### Policy Decisions

Each change group receives one of three policy decisions:

- **AutoCommittable** - Safe to automatically commit
- **RequiresReview** - Needs human review before commit
- **Blocked** - Should not be committed (requires attention)

## Requirements

- [Git](https://git-scm.com/) - AVCS uses git as the underlying VCS
- Rust 2021 edition or later

## Architecture

```
src/
├── cli/           # Command-line interface
├── core/          # Core types and output rendering
├── intent/        # Intent classification and clustering
│   ├── classifier/   # Change type classification
│   ├── cluster/     # File clustering and grouping
│   ├── evidence/    # Evidence detection modules
│   ├── message/     # Commit message generation
│   └── policy/      # Policy evaluation engine
├── vcs/           # Git integration via git2 crate
└── semantic/      # Semantic analysis with tree-sitter
```

## License

MIT
