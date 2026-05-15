# Building & Development

**Status:** Current
**Last updated:** 2026-04-07 06:29 EDT

Development is supported on **Windows, macOS, and Linux**. The instructions below use Unix shell syntax; on Windows, use PowerShell or Git Bash equivalently.

## Prerequisites

- **[uv](https://docs.astral.sh/uv/)** -- Python package manager (all platforms). Used for all dependency management and running commands.
- **Rust (stable)** via [rustup](https://rustup.rs/) (all platforms) -- needed for the Rust CLI and PyO3 extension.
- **Node.js + npm** -- needed for `bazel build //...` and `bazel build //...-dashboard`, which rebuild the embedded dashboard bundled into the Rust binary.
- **`cargo-nextest`** -- Required for Rust test runs. Install once with `cargo install cargo-nextest --locked`.
- **[maturin](https://www.maturin.rs/)** -- Required only if you modify the Rust `batchalign_core` extension.
- **Python 3.12** for development and current deployment targets. 3.14t/free-threaded work is paused again and is **not** an active install or deployment target. Revisit only when `developer/python-versioning.md` is updated for a newer interpreter line such as 3.15+.
- **Platform note:** On macOS, `python` and `python3` may not exist outside a venv. Always use `uv run` to execute Python commands, which handles this automatically on all platforms.

## Development Install

Batchalign3's Rust crates depend on [`talkbank-tools`](https://github.com/talkbank/talkbank-tools) via local path references. Both repos must be cloned as siblings:

```bash
git clone https://github.com/talkbank/talkbank-tools.git
git clone https://github.com/talkbank/batchalign3.git
cd batchalign3
make sync
bazel build //...
```

If you do not need the dashboard build during iteration, you can rebuild just
the Rust/PyO3 surfaces with `bazel build //...-python` and `bazel build //...-rust`.
For the fastest contributor loop, `bazel build //...-python` rebuilds only the PyO3
extension. `bazel build //...-python-full` also copies the pre-built CLI binary into
`python/batchalign/_bin/` so `uv run batchalign3` uses the packaged binary instead of
the dev fallback.

The expected directory layout:

```text
parent/
├── talkbank-tools/    # CHAT grammar, parser, model, transform crates
└── batchalign3/       # This repo (Rust CLI + server + Python ML workers)
```

This creates a `.venv` managed by uv. Never use `pip install` directly.

`make sync` provisions the same built-in engine surface as the base package,
including Cantonese providers. There is no separate Cantonese-specific dev
extra path.

## Running the CLI

In a source checkout, `uv run batchalign3` is still the normal way to invoke
the installed console script. After `bazel build //...-python`, the Python wrapper
falls back to the repo CLI when the embedded bridge is intentionally omitted,
so the fast extension-only rebuild still leaves you with a runnable
`batchalign3` command. This is the recommended loop while editing command
semantics, workflow families, or most docs.

For the fastest contributor loop, pair `bazel build //...-python` with one CLI build
up front:

```bash
cargo build -p batchalign-cli
```

After that, repeated `uv run batchalign3 ...` invocations will use the local
`target/debug/batchalign3` binary through the wrapper fallback. Reserve
`uv run` for Python tools such as `pytest`, `mypy`, and `maturin` when you are
not invoking the CLI.

```bash
bazel build //...
./target/debug/batchalign3 --help
./target/debug/batchalign3 transcribe input_dir -o output_dir --lang eng
./target/debug/batchalign3 morphotag input_dir -o output_dir
./target/debug/batchalign3 align input_dir -o output_dir

# Or let Cargo rebuild the Rust binary incrementally for you:
cargo run -p batchalign-cli -- transcribe input_dir -o output_dir --lang eng
```

## What to Rebuild After Changes

Use the repo-native build targets so the Rust CLI, the shared `batchalign`
crate, and the `batchalign_core` extension stay in sync:

| What changed | What to rebuild |
| --- | --- |
| Python code only (`batchalign/`) | Nothing; the next worker process picks up the change |
| Rust CLI / server (`crates/batchalign/`, `crates/batchalign/`) | `cargo build -p batchalign-cli` or `bazel build //...-rust` |
| Shared chat logic (`crates/batchalign/`) or PyO3 bridge (`crates/batchalign-pyo3/`) | `bazel build //...-python`; for the fastest CLI loop in a source checkout, also build the CLI once (`cargo build -p batchalign-cli` or `bazel build //...-rust`) so the wrapper can fall back to `target/debug/batchalign3` |
| Command/orchestrator changes (`crates/batchalign/src/commands/`, `compare.rs`, `benchmark.rs`, `transcribe/`, `fa/`, `morphosyntax/`, `command_family.rs`, `text_batch.rs`) | `bazel build //...-rust` and usually `bazel build //...-python` if the CLI bridge surface changed |
| Cross-cutting or dashboard changes | `bazel build //...` (requires Node.js + npm because it rebuilds the embedded dashboard) |

## Rebuilding the Rust Extension

The `batchalign_core` Python package is a PyO3 Rust extension built by maturin.
The repo-native rebuild path is:

```bash
bazel build //...-python
```

This rebuilds only the PyO3 worker runtime extension (~320 crates). The pyo3
crate has no feature gates beyond `extension-module` — no heavy CLI or Rev.AI
dependencies. In a source checkout, `batchalign/_cli.py` falls back to
`target/debug/batchalign3` when the packaged binary isn't present.

When you want the CLI binary packaged alongside the extension:

```bash
bazel build //...-python-full
```

That target first builds the Rust CLI binary, copies it to `python/batchalign/_bin/`,
then rebuilds the extension. Use it when testing the installed-package
experience locally.

## CLI Binary Packaging (`python/batchalign/_bin/`)

batchalign3 ships two native artifacts in its wheel:

1. **`batchalign_core.so`** — the PyO3 extension (gives Python access to Rust
   CHAT parsing, alignment, etc.)
2. **`python/batchalign/_bin/batchalign3`** — the standalone Rust CLI binary (the
   server, job runner, and all commands)

The Python entry point (`batchalign/_cli.py`) locates and execs the native CLI
binary. It searches three locations in order:

1. **Packaged binary** at `python/batchalign/_bin/batchalign3` — this is what PyPI
   users get. The binary is bundled inside the wheel.
2. **Dev checkout** at `target/{debug,release}/batchalign3` — for developers
   who built the CLI with `cargo build`.
3. **Cargo fallback** — execs `cargo run -p batchalign-cli` to compile on the
   fly.

### Why `_bin/` is gitignored

The binary is a 50+ MB platform-specific build artifact — it must not be
tracked in git. Instead:

- **Locally:** `bazel build //...-python-full` compiles the CLI and copies it into
  `python/batchalign/_bin/`. Most developers skip this and rely on the dev-checkout
  fallback (`target/debug/batchalign3`).
- **CI:** A dedicated `build-cli` job compiles the CLI binary once (release
  mode), uploads it as an artifact, and each Python-version wheel build
  downloads it into `python/batchalign/_bin/` before maturin packages it.
- **Release:** The release workflow builds platform-specific CLI binaries
  (macOS ARM + Intel, Linux x86 + ARM, Windows x86) and packages each into
  the corresponding wheel.

### Maturin include directive

`pyproject.toml` tells maturin to include the binary in the wheel:

```toml
[tool.maturin]
include = [
    { path = "python/batchalign/_bin/batchalign3", format = "wheel" },
    { path = "python/batchalign/_bin/batchalign3.exe", format = "wheel" },
]
```

If the binary doesn't exist at build time, maturin silently skips it — the
wheel still builds but `batchalign3 --help` will fail at runtime with
"CLI binary not found." This is why CI must build and copy the binary
**before** running `uv build --wheel`.

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
