# Installation

**Status:** Current
CLAN commands are part of the `chatter` CLI tool.

## From source

```bash
# Clone the repository
git clone https://github.com/TalkBank/batchalign.git

# Build
cd batchalign
cargo install --path crates/chatter/chatter-cli
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
