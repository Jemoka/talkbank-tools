# Maturin Build and PyO3 Dependency Surface

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## Overview

The `batchalign_core` Python extension is built by **maturin** from `crates/batchalign-pyo3/Cargo.toml`.
The crate has exactly one feature gate: `extension-module` (required by PyO3 for
cdylib linking). No other features exist — the extension is always slim.

## Dependency Graph

```text
batchalign-pyo3 (the .so, ~2,730 lines, 9 files)
  |
  +-- batchalign-types          (newtypes, worker IPC types)
  +-- batchalign       (Cantonese ASR projection, Cantonese normalization,
  |                              tokenizer realignment, text task normalization)
  +-- pyo3, numpy, serde, serde_json, tracing
```

That's it. ~319 crates in the full dependency tree. No server, no CLI, no
Rev.AI, no talkbank-model, no talkbank-parser.

### Why each dependency exists

| Crate | Used by pyo3 for | Could be removed? |
|-------|-------------------|-------------------|
| `batchalign-types` | Domain newtypes, worker IPC types (`ExecuteRequestV2`, etc.) | No — core shared types |
| `batchalign` | Cantonese ASR projection, Cantonese normalization, tokenizer realignment, coref types, text result normalization | No — worker-side Rust logic |
| `pyo3` / `numpy` | PyO3 bridge, NumPy array handling for audio | No — fundamental |
| `serde` / `serde_json` | JSON serialization for IPC | No — fundamental |

### What was removed

| Removed dep | Why |
|-------------|-----|
| `batchalign` (+ transitive `batchalign`) | CLI binary shipped as package data instead of compiled into .so |
| `batchalign-revai` | Dead code — server uses Rev.AI directly |
| `talkbank-model` | Only used by deleted ParsedChat class |
| `talkbank-parser` | Only used by deleted parse helpers |
| `indexmap`, `thiserror` | Only used by deleted standalone functions |

## CLI Binary Distribution

The `batchalign3` CLI is a standalone Rust binary (`crates/batchalign`).
It is **not** compiled into the .so extension. Instead:

- **PyPI wheels**: The binary is pre-built and included as package data at
  `python/batchalign/_bin/batchalign3`. The console_scripts entry point
  (`batchalign/_cli.py`) finds and execs it.
- **Dev checkout**: `_cli.py` falls back to `target/debug/batchalign3` or
  `cargo run -p batchalign-cli`.

This eliminates the old `cli-entry` feature gate that dragged 741 extra crates
(the entire server stack) into the extension build.

The wrapper is intentionally thin, but it does carry two load-bearing runtime
handoffs into the Rust binary:

- `BATCHALIGN_PYTHON` — preserve the interpreter/venv that owns the installed
  worker package
- `BATCHALIGN_SELF_EXE` — preserve the actual packaged Rust binary path so
  server/daemon re-exec paths do not have to infer it from the Python
  console-script launcher

## Build Commands

```bash
# Development rebuild (debug, fast — ~7s incremental)
bazel build //...-python
# equivalent: uv run maturin develop -m crates/batchalign-pyo3/Cargo.toml -F pyo3/extension-module

# Full package (extension + CLI binary in python/batchalign/_bin/)
bazel build //...-python-full

# Release wheel for deployment
cargo build --release -p batchalign-cli --bin batchalign3
cp target/release/batchalign3 python/batchalign/_bin/batchalign3
uv run maturin build --release -m crates/batchalign-pyo3/Cargo.toml -F pyo3/extension-module --out dist/

# Check compilation without building wheel
cargo check --manifest-path crates/batchalign-pyo3/Cargo.toml
```

## What NOT to do

- **Do not add server deps to pyo3.** The extension is for the worker process.
  If the server needs Rust functionality, use `batchalign` or
  `batchalign` directly — not through pyo3.

- **Do not vendor types.** Use path dependencies. `batchalign-types` is the
  single source of truth for domain newtypes and worker IPC types.

- **Do not add feature gates.** The extension should always build the same way.
  If something is optional, it probably doesn't belong in pyo3.

- **Do not compile the CLI into the .so.** The binary is shipped as package
  data. If you need to change how the CLI is invoked, modify `_cli.py`.

- **Do not move orchestration into `_cli.py`.** The wrapper may pass runtime
  hints into Rust, but the actual CLI/server behavior still belongs to the Rust
  binary.

## Verification checklist

After any dependency change to `crates/batchalign-pyo3/Cargo.toml`:

```bash
cargo check --manifest-path crates/batchalign-pyo3/Cargo.toml
cargo nextest run --manifest-path crates/batchalign-pyo3/Cargo.toml
uv run maturin develop -m crates/batchalign-pyo3/Cargo.toml -F pyo3/extension-module
uv run batchalign3 --help
uv run pytest
```
