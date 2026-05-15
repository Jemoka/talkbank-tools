# batchalign-core — Rust Worker Runtime

**Status:** Current
**Last modified:** 2026-05-01 09:47 EDT

## Overview

Slim PyO3 bridge providing the Rust worker runtime for batchalign3's Python ML
worker processes. Workers are stateless inference endpoints that load ML models,
receive structured data from the Rust server via stdio JSON-lines IPC, run
inference, and return raw results.

This crate does NOT contain CHAT parsing, AST manipulation, or pipeline
orchestration — all of that lives in the Rust server (`crates/batchalign/`)
and `batchalign`.

This crate is a regular workspace member at `crates/batchalign-pyo3/`.
The maturin build drives it via
`--manifest-path crates/batchalign-pyo3/Cargo.toml` (the cdylib half of
`crate-type = ["cdylib", "rlib"]` is what Python imports as
`batchalign_core`); `cargo ... --workspace` builds the rlib half and
runs its tests like any other crate.

## Layout

```
crates/batchalign-pyo3/src/
├── lib.rs                  # Module registration
├── cli_entry.rs            # PyPI console_scripts entry point
├── worker_protocol.rs      # IPC message dispatch
├── worker_asr_exec.rs      # ASR execution (Whisper, Cantonese providers)
├── worker_fa_exec.rs       # Forced alignment execution
├── worker_media_exec.rs    # Speaker diarization, OpenSMILE, AVQI
├── worker_text_results.rs  # Text task normalization + token alignment
├── worker_artifacts.rs     # Prepared artifact loading from IPC
├── cantonese_asr_bridge.rs        # Cantonese provider projection + normalization
└── py_json_bridge.rs       # Python→JSON conversion utility
```

## Key Commands

```bash
cargo nextest run --manifest-path pyo3/Cargo.toml
cargo build --manifest-path pyo3/Cargo.toml
cd /path/to/batchalign3 && uv run maturin develop
```

## Rust Coding Standards

See root `CLAUDE.md` for workspace-universal Rust standards (edition, error
handling, logging, file size limits, git conventions). This crate follows all
of those. Crate-specific additions below.

## Rules

- **All JSON via serde.** `#[derive(Deserialize)]`/`#[derive(Serialize)]` structs only.
- **GIL release.** All pure-Rust methods use `py.detach()` (pyo3 0.28).
- **No CHAT parsing here.** CHAT manipulation is in `batchalign` and
  the Rust server. This crate only bridges Python ML calls.

## Architecture

```
Rust Server (crates/batchalign/)
  ├── Parses CHAT, extracts payloads
  ├── Sends IPC request to Python worker (stdio JSON-lines)
  │
  └── Python Worker Process
        ├── worker_protocol.rs: dispatch IPC messages
        ├── worker_*_exec.rs: load prepared artifacts, call ML model
        ├── cantonese_asr_bridge.rs: project Cantonese provider output
        └── Returns raw results → Rust server injects into CHAT
```

**See also:** [Interface Map](../../INTERFACE_MAP.md) for unified documentation of all
Python/Rust boundaries, including Python caller locations, shared schema
definitions, and responsibility splits per boundary.
