# send2clan-sys

Rust FFI bindings for [send2clan](https://talkbank.org/) — enables sending file open messages to the CLAN (Computerized Language Analysis) application.

## Overview

This crate provides idiomatic Rust bindings around the C `send2clan` library.
When invoked, it launches CLAN (if not already running) and sends a file path
with cursor position and optional error text so the user can jump directly to
the relevant location.

- **Cross-platform**: macOS and Windows (no-op on other platforms)
- **Stateless**: no context or configuration management needed
- **Thread-safe**: can be called from multiple threads simultaneously

## Usage

```rust,no_run
use send2clan::send_to_clan;

fn main() -> Result<(), send2clan::Error> {
    // Send file to CLAN with 30-second timeout
    send_to_clan(30, "/path/to/file.cha", 42, 10, Some("Syntax error"))?;
    println!("Successfully sent file to CLAN!");
    Ok(())
}
```

Helper functions:

- `is_platform_supported()` — check if the current OS is supported
- `is_clan_available()` — check if CLAN is installed
- `version()` — get library version string
- `get_capabilities()` — query runtime capabilities

## Build Requirements

A C compiler is required (the crate compiles the bundled C source via `cc`).

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
