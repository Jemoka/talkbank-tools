# Setup

**Status:** Current
**Last updated:** 2026-03-24 00:01 EDT

Development is supported on **Windows, macOS, and Linux**. The instructions below use Unix shell syntax; on Windows, use PowerShell or Git Bash equivalently.

## Prerequisites

- **Rust (stable)** via [rustup](https://rustup.rs/) (all platforms)
- **Node.js** for tree-sitter grammar generation and symbol validation
- **tree-sitter CLI**: `cargo install tree-sitter-cli`

## Clone Repository

```bash
mkdir -p ~/talkbank && cd ~/talkbank
git clone <talkbank-tools-url> talkbank-tools
```

## Build

```bash
cd ~/talkbank/talkbank-tools
cargo build               # Build all crates
cargo build --all-targets # Including tests and benchmarks
```

## Two Cargo Workspaces

The repository has two independent Cargo workspaces:

### 1. Root workspace (`Cargo.toml`)

Contains all Rust crates for parsing, model, validation, and transform:

```bash
cd ~/talkbank/talkbank-tools
cargo build
cargo test
```

### 2. Spec workspace (`crates/spec/talkbank-spec-testgen/Cargo.toml`)

Contains two sibling crates for spec-driven artifacts:

```bash
cargo build --manifest-path ~/talkbank/talkbank-tools/crates/spec/talkbank-spec-testgen/Cargo.toml
cargo build --manifest-path ~/talkbank/talkbank-tools/crates/spec/talkbank-spec-testrun/Cargo.toml
cargo run --manifest-path ~/talkbank/talkbank-tools/crates/spec/talkbank-spec-testgen/Cargo.toml --bin gen_tree_sitter_tests -- --help
cargo run --manifest-path ~/talkbank/talkbank-tools/crates/spec/talkbank-spec-testrun/Cargo.toml --bin validate_error_specs -- --help
```

## Makefile Targets

```bash
bazel build //...           # Build everything
bazel test //...            # Run all tests (nextest + parser-tests + doctests)
bazel build //... && bazel test //...          # Pre-merge verification gates
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs        # Regenerate spec-driven artifacts when they actually changed
node resources/spec/symbols/validate_symbol_registry.js && node scripts/generate-symbol-sets.js && node resources/spec/symbols/generate_rust_symbol_sets.js     # Regenerate shared symbol sets
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code # Verify generated artifacts are committed
bazel build //...           # Fast compile check
bazel clean           # Clean build artifacts
just docs build            # Build documentation
just docs serve      # Serve documentation locally
```

## Verification

Before submitting changes, run the full verification suite:

```bash
bazel build //... && bazel test //...
```

See [Testing](testing.md) for the current gate breakdown. The important point is
that `bazel build //... && bazel test //...` remains the pre-merge gate, while `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` is a
targeted regeneration step rather than a universal parser-testing ritual.

## Editor Setup

### VS Code

Install the TalkBank extension from `apps/vscode-extension/` for CHAT syntax highlighting and diagnostics.

### rust-analyzer

The workspace should work out of the box with rust-analyzer. The root `Cargo.toml` workspace configuration is standard.
