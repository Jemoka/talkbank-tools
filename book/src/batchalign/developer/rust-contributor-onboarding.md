# Rust Contributor Onboarding

**Status:** Current
**Last updated:** 2026-03-21 07:16 EDT

This page is the shortest path to productive work on the Rust side of Batchalign3.

## Start Here

1. Read the user-facing [CLI reference](../user-guide/cli-reference.md).
2. Read the [Rust workspace map](rust-workspace-map.md).
3. Read the [Rust CLI and Server](rust-cli-and-server.md) for dispatch architecture and command-creation checklist.
4. Read the [migration book](../migration/index.md) if you need historical context from Batchalign2.
5. Run the root workspace tests before changing behavior.

## Current Rust Surfaces

- root workspace:
  - `batchalign`
  - `batchalign`
  - `batchalign`
- PyO3 bridge:
  - `crates/batchalign-pyo3/` building `batchalign_core`

## Setup

```bash
make sync
bazel build //...
cargo check --workspace
cargo nextest run --workspace
cargo nextest run --manifest-path crates/batchalign-pyo3/Cargo.toml
```

`make sync` is the normal setup path even for Cantonese/provider work.
Cantonese ASR engines are part of the main package surface, not a separate
extra or plugin tier.

Rebuild rule of thumb while iterating:

- CLI/server-only changes: `cargo build -p batchalign-cli` or `bazel build //...-rust`
- `batchalign` or `crates/batchalign-pyo3/` changes: `bazel build //...-python`
- the fast contributor loop: run `cargo build -p batchalign-cli` once, then
  `uv run batchalign3 ...` will use the repo CLI fallback in a source checkout
  after a slim `bazel build //...-python`

## Where To Work

- CLI flags, logs, cache, daemon behavior: `crates/batchalign`
- server routes, jobs, persistence, OpenAPI: `crates/batchalign`
- shared CHAT transformations and mapping logic: `crates/batchalign`
- Python extension boundary: `crates/batchalign-pyo3/`

## Expectations

- add or update tests before large behavioral changes
- keep public docs in sync with the actual CLI and server surface
- do not introduce maintainer-local filesystem paths into public docs
- treat migration notes as historical context, not as the current API contract

## Useful Commands

```bash
cargo build -p batchalign-cli
bazel build //...-python
cargo nextest run -p batchalign-cli --test cli
cargo nextest run -p batchalign-cli --test e2e
cargo nextest run -p batchalign-cli --test integration
cargo nextest run --manifest-path crates/batchalign-pyo3/Cargo.toml
cargo run -q -p batchalign-cli -- openapi --check --output openapi.json
```
