# Installation

**Status:** Current
CLAN commands are part of the `chatter` CLI tool.

## From source

```bash
# Clone the repository
git clone https://github.com/TalkBank/talkbank-tools.git

# Build
cd talkbank-tools
cargo install --path crates/chatter-cli
```

## Verify installation

```bash
chatter clan --help
chatter clan freq --help
```

## Requirements

- Rust 2024 edition (rustc 1.85+)
- macOS, Linux, or Windows
- No runtime dependencies beyond the binary
