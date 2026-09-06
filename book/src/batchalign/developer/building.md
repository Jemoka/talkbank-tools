# Building & Development

**Status:** Current
**Last updated:** 2026-05-31 18:00 EDT

Development is supported on **Windows, macOS, and Linux**. The instructions
below use Unix shell syntax; on Windows, use PowerShell or Git Bash
equivalently.

## Prerequisites

- **[uv](https://docs.astral.sh/uv/)** — Python package manager. Used for all
  Python dependency management and running the CLI.
- **Rust ≥ 1.95** via [rustup](https://rustup.rs/) — needed because the
  workspace lockfile pins `sqlx 0.9` and `sysinfo 0.39`, both of which
  require recent rustc. `rustup install 1.95.0` then either
  `RUSTUP_TOOLCHAIN=1.95.0` or `rustup override set 1.95.0`.
- **Bazel (via the `tools/bazel` wrapper)** — canonical build system. The
  repo ships a checked-in wrapper, so a separate Bazel install is not
  required.
- **`cargo-nextest`** — required for Rust test runs.
  `cargo install cargo-nextest --locked`.
- **[maturin](https://www.maturin.rs/)** — only if you modify the
  `batchalign_core` PyO3 extension.
- **Node.js + npm** — only if you touch the embedded dashboard
  (`apps/batchalign/batchalign-gui/`).
- **Python 3.12** for development and current deployment targets.

This repository is a single monorepo — there is no sibling `batchalign3`
checkout. The Rust crates live under `crates/batchalign/`, the Python
package under `python/batchalign/`, and the desktop GUI under
`apps/batchalign/batchalign-gui/`.

## Development Install

```bash
git clone https://github.com/TalkBank/batchalign.git
cd batchalign
cd python && uv sync --group dev && cd ..   # provisions python/.venv
bazel build //...                           # full workspace
```

For day-to-day iteration, the canonical surfaces are:

- `bazel build //...` — full workspace (Rust + Python wheels + GUI bundle).
- `bazel test //...` — full test suite (pre-merge gate).
- `cargo build --release -p batchalign-core --bin emit_proto_schema` —
  the only Rust binary in `batchalign-core` is the proto schema emitter;
  the user-facing `batchalign3` CLI is a Python Typer app
  (`python/pyproject.toml: batchalign3 = "batchalign.cli:app"`).

To run the CLI from the source tree:

```bash
uv run --project python batchalign3 --help
```

## Running the CLI

The CLI is a Python Typer app distributed via uv:

```bash
uv run --project python batchalign3 --help
uv run --project python batchalign3 transcribe input_dir -o output_dir --lang eng
uv run --project python batchalign3 morphotag input_dir -o output_dir
uv run --project python batchalign3 align input_dir -o output_dir
uv run --project python batchalign3 version       # banner + git SHA + maintainers
```

Internally the CLI dispatches NLP work to the PyO3 `batchalign_core`
extension (built by maturin) and orchestrates worker processes from
Python. There is no standalone `batchalign-cli` Rust crate in this
repository.

## What to Rebuild After Changes

| What changed | What to rebuild |
| --- | --- |
| Python code only (`python/batchalign/`) | Nothing; `uv run` picks it up |
| `crates/batchalign/batchalign-core/` (proto + types) | `bazel build //crates/batchalign/batchalign-core/...` |
| `crates/batchalign/batchalign-engine/` (PyO3 cdylib) | `bazel build //crates/batchalign/batchalign-engine/...` |
| `crates/core/talkbank-*` (parser, model, transform) | `bazel build //crates/core/...` |
| Embedded GUI (`apps/batchalign/batchalign-gui/`) | `bazel build //apps/batchalign/batchalign-gui/...` |
| Cross-cutting | `bazel build //...` |

For the fast PyO3 iteration loop in a source checkout you can also use
maturin directly:

```bash
RUSTUP_TOOLCHAIN=1.95.0 uv run --project python maturin develop \
    -m crates/batchalign/batchalign-pyo3/Cargo.toml \
    -F pyo3/extension-module
```

This rebuilds only the PyO3 worker runtime extension and installs it
editable into `python/.venv`.

## CLI Binary Packaging (`python/batchalign/_bin/`)

batchalign3 ships one native artifact in its wheel:

**`batchalign_core.so`** — the PyO3 extension (gives Python access to Rust
CHAT parsing, alignment, transform, and engine orchestration). Built by
maturin from `crates/batchalign/batchalign-engine`.

The `batchalign3` console-script entry point is the Python Typer app
(`python/pyproject.toml: batchalign3 = "batchalign.cli:app"`); there is
no separate Rust binary to bundle. NLP work is dispatched into Python
worker subprocesses; heavy chat/parsing/transform work is offloaded
through the PyO3 extension.

(Franklin's fork ships a standalone Rust `batchalign3` binary at
`python/batchalign/_bin/batchalign3`; we deliberately don't.)

## Where Command Logic Should Live

If you are changing command behavior, the first stop should be the owning
command module in `crates/batchalign/src/commands/` and then the module
that actually owns the algorithmic or orchestration semantics (`compare.rs`,
`benchmark.rs`, `transcribe/`, `fa/`, `morphosyntax/`, etc.).

- `crates/batchalign/src/commands/` owns released-command identity, specs,
  and the top-level contributor-facing entrypoints.
- `crates/batchalign/src/command_family.rs` keeps the small command-shape
  enum used by command metadata.
- `crates/batchalign/src/text_batch.rs` keeps reusable text-batch helper
  types for commands such as `utseg`, `translate`, and `coref`.
- `crates/batchalign/src/runner/` owns job lifecycle, queueing, and shared
  dispatch machinery.
- `crates/batchalign/src/dispatch/` should stay thin and focus on
  argument parsing, capability gating, and whether a command runs locally or
  through the server.
- `crates/batchalign-pyo3/` should stay a thin bridge, not the place where new command logic is
  invented.

Run the Rust test suite to verify your changes:

```bash
cargo nextest run --manifest-path crates/batchalign-pyo3/Cargo.toml
```

## Type Checking

Run the current mypy gate before every commit:

```bash
uv run mypy
# or together with clippy:
make lint
```

Strictness lives in `mypy.ini`, and CI runs the same repo-native command shape.

Do not commit with mypy errors. Use `# type: ignore[<code>]` only when
necessary, and always include the specific error code.

## Type Annotation Rules

All new and modified code must include type annotations:

- Annotate all function parameters and return types.
- Use modern syntax: `list[str]` not `List[str]`, `str | None` not `Optional[str]`.
- **`Any` and `object` are banned as type annotations.** Use specific types. For ML library types that are expensive to import, use `TYPE_CHECKING` guards with the real type.
- Use `from __future__ import annotations` for forward references where needed.
- Prefer `TYPE_CHECKING` imports for heavy dependencies used only in annotations.

## The CHAT Format Rule

All CHAT parsing and serialization must go through principled AST manipulation via `batchalign_core` Rust functions. This is a hard rule with no exceptions.

**Do not:**
- Use regex or string splitting to extract or modify CHAT content.
- Process CHAT line-by-line in Python.
- Manipulate CHAT header metadata with ad-hoc text code.

**Instead:**
- Use existing `batchalign_core` functions (`parse`, `parse_lenient`, `build_chat`, `add_morphosyntax`, `add_forced_alignment`, `extract_nlp_words`, etc.).
- If the function you need does not exist, add a new Rust function to `batchalign_core` and call it from Python.

CHAT has complex escaping, continuation lines, and encoding rules that ad-hoc text manipulation will get wrong. The Rust AST handles all of this correctly.
