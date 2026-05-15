# Installation

**Status:** Current
**Last updated:** 2026-04-28 23:14 EDT

`chatter` runs on **Windows, macOS, and Linux**. Pre-built binaries are
available from the [GitHub Releases](https://github.com/TalkBank/talkbank-tools/releases)
page; choose the plain `vX.Y.Z` TalkBank core release entry and its
`chatter-v...` assets (VS Code preview releases use `vscode-vX.Y.Z` tags in the
same repository). To build from source, follow the instructions below.

## Prerequisites

- **Rust (stable)** — install via [rustup](https://rustup.rs/) (supports Windows, macOS, Linux)

If you are only building the `chatter` CLI from source, Rust is sufficient.
The extras below are only needed when you work on the grammar or generated
artifacts:

- **Node.js** — required for tree-sitter grammar generation
- **tree-sitter CLI** — `cargo install tree-sitter-cli`

## Installing chatter (CLI)

The `chatter` CLI is the primary tool for working with CHAT files. It provides validation, format conversion, and batch processing.

### From Source

Clone the repository:

```bash
mkdir -p ~/talkbank && cd ~/talkbank
git clone https://github.com/TalkBank/talkbank-tools.git talkbank-tools
```

Build and install:

```bash
cd ~/talkbank/talkbank-tools
cargo install --path crates/chatter-cli
```

This installs the `chatter` binary to `~/.cargo/bin/` (macOS/Linux) or `%USERPROFILE%\.cargo\bin\` (Windows).

### Verify Installation

```bash
chatter --version
chatter --help
```

## Building the Libraries

If you're developing with the Rust crates directly:

```bash
cd ~/talkbank/talkbank-tools
cargo build           # Build all crates
cargo test            # Run all tests
cargo clippy          # Lint check
```

See the [Makefile targets](../../contributing/setup.md) for additional commands.

## Directory Layout

Everything lives in a single repository:

```text
~/talkbank/
└── talkbank-tools/         # This repo (grammar, crates, CLI, LSP, VS Code, CLAN, Batchalign)
    ├── grammar/            # Tree-sitter grammar
    ├── crates/             # All Rust crates (talkbank-* and batchalign-*)
    ├── spec/               # CHAT specification
    ├── apps/vscode-extension/             # VS Code extension
    ├── apps/               # Tauri desktop apps (chatter-gui, dashboard-desktop) — both experimental
    └── book/               # TalkBank Toolchain mdBook
```

The CLI, grammar, crates, Batchalign pipeline, and all related tools are in this single repository.
