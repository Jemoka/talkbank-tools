# Rust Core (`batchalign_core`)

**Status:** Current
**Last updated:** 2026-05-01 22:47 EDT

For new contributors, start with:

- [Rust Contributor Onboarding](rust-contributor-onboarding.md)
- [Rust Workspace Map](rust-workspace-map.md)

## Repository structure

The PyO3 bridge lives at `crates/batchalign-pyo3/` inside the
`talkbank-tools` workspace. It's a single-crate project that builds
the `batchalign_core` Python extension module that worker processes
import. It depends on the sibling `batchalign` and `batchalign-types`
crates.

Editable installs use the maturin build backend declared in
`pyproject.toml` (`build-backend = "maturin"`, `[tool.maturin]
manifest-path = "crates/batchalign-pyo3/Cargo.toml"`, feature
`pyo3/extension-module`, `profile = "dev"` for fast rebuild).
For a packaged install you build the wheel via `make
batchalign-build-wheel`, which compiles `batchalign3` in release
mode, copies it into `python/batchalign/_bin/`, and then runs `uv build
--wheel --out-dir dist/` to produce the maturin-built wheel that
bundles both the extension and the native binary.

| Crate | Location | Purpose |
|-------|----------|---------|
| `batchalign-pyo3` | `crates/batchalign-pyo3/` | Worker runtime — what Python imports as `batchalign_core` |
| `batchalign` | `crates/batchalign/` | The big Rust crate: CLI, axum server, dispatch, FA, morphosyntax, Rev.AI client, CHAT extraction/injection |
| `batchalign-types` | `crates/batchalign-types/` | Shared domain types and worker IPC types |

## Module organization

The PyO3 crate is a slim worker runtime. All CHAT manipulation lives
in the sibling `batchalign` crate and is called directly by the Rust
runtime — never via callbacks from Python.

| Module | Purpose |
|--------|---------|
| `lib.rs` | Module registration |
| `worker_protocol.rs` | IPC message dispatch (health, capabilities, infer, execute_v2) |
| `worker_asr_exec.rs` | ASR execution (Whisper, Cantonese providers) |
| `worker_fa_exec.rs` | Forced alignment execution |
| `worker_text_results.rs` | Text task normalization + `align_tokens` |
| `worker_artifacts.rs` | Prepared artifact loading from IPC attachments |
| `cantonese_asr_bridge.rs` | Cantonese provider projection + normalization |
| `py_json_bridge.rs` | Python ↔ JSON conversion utility |

For exact line counts and the current module-by-module shape, read
the source under `crates/batchalign-pyo3/src/`.

## Key PyO3 entry points

### Worker protocol

- `dispatch_protocol_message(...)` — route IPC messages to typed
  Python handlers.

### Worker V2 executors

| Function | Purpose |
|----------|---------|
| `execute_asr_request_v2(...)` | Load prepared audio, call Whisper / Cantonese provider |
| `execute_forced_alignment_request_v2(...)` | Load prepared audio + text, call FA model |
| `execute_speaker_request_v2(...)` | Load prepared audio, call pyannote / NeMo |
| `normalize_text_task_result(...)` | Reshape `BatchInferResponse` → typed V2 results |

### Utilities

| Function | Purpose |
|----------|---------|
| `align_tokens(...)` | Map Stanza tokenizer output back to CHAT words |
| `normalize_cantonese(...)` | Simplified → traditional + domain replacements |
| `cantonese_char_tokens(...)` | Per-character tokenization for Cantonese FA |
| Cantonese bridge functions | Project FunASR / Tencent / Aliyun output into common shapes |

These functions are internal — they exist for the Rust runtime to
call into the worker process. They are not part of any public API
surface and external code should not import them.

## What was removed (historical context)

The PyO3 surface previously exposed a much larger set of callback
methods and standalone Python-facing functions. They were removed as
the Rust runtime grew enough to own the CHAT lifecycle directly:

- `ParsedChat` class and all its callback methods (`parse`,
  `serialize`, `add_morphosyntax`, `add_forced_alignment`, etc.)
- `run_provider_pipeline()` and provider-pipeline helpers
- Standalone functions: `build_chat`, `parse_and_serialize`,
  `extract_nlp_words`, `wer_compute`, `wer_metrics`, `dp_align`, etc.
- All inner-function modules: `morphosyntax_ops`, `fa_ops`,
  `text_ops`, `speaker_ops`, `cleanup_ops`, `tier_ops`

All domain logic now lives in the Rust `batchalign` crate (with
inline tests there) and is exercised through the Rust runtime.

## Tree-sitter grammar

The CHAT grammar lives in this same workspace at `grammar/`. After
editing `grammar/grammar.js`, regenerate the C parser:

```bash
cd grammar && tree-sitter generate
```

This regenerates `parser.c`, which the tree-sitter parser depends on.
**Forgetting this step causes the parser to use a stale grammar.**

After grammar changes, always test against real corpus data in
addition to the curated test suite. See the parent
`talkbank-tools/CLAUDE.md` "Grammar Change Workflow" section for the
full mandatory sequence.

## Building for development

When you change the PyO3 bridge or the shared Rust logic that
worker processes consume, rebuild the extension and reinstall it
into the dev environment:

```bash
just batchalign develop
```

This depends on `batchalign-build-wheel`, which rebuilds the native
`batchalign3` binary in release mode, copies it into
`python/batchalign/_bin/`, and produces a maturin-built wheel via `uv
build --wheel`. The prepare target then runs `uv sync --group dev
--no-install-project` and `uv pip install --reinstall --no-deps
dist/*.whl` to install the freshly built wheel into the active
`uv` environment.

For day-to-day editable iteration on the PyO3 layer, plain
`uv run <anything>` will trigger an incremental rebuild against
`pyproject.toml`'s maturin backend (`profile = "dev"`).

If you also plan to run the standalone Rust CLI directly after a
shared-crate change, rebuild that binary too:

```bash
cargo build -p batchalign-cli
```

## Running Rust tests

```bash
cargo nextest run --manifest-path crates/batchalign-pyo3/Cargo.toml
```

For the parser integration suite, run from the workspace root:

```bash
cargo nextest run -p talkbank-parser-tests
```

## GIL release strategy

All pure-Rust `batchalign_core` entry points release the Python GIL
via `py.detach()` (pyo3 0.28). This lets other Python threads run
while Rust does CPU-bound work.

The few entry points that take Python callbacks hold the GIL only
during the callback invocation; outside of that they release. The
pattern is:

1. Release GIL, walk Rust data, collect inputs.
2. Acquire GIL, call the Python callback.
3. Release GIL, process the callback's result.

This means CPU-bound Rust work doesn't block other Python threads,
while the callback (which runs Python model inference) holds the GIL
as expected.

## Workflow: adding a new worker-side capability

The "add a new CHAT transformation" workflow used to involve adding
`#[pymethods]` on a `ParsedChat` class that no longer exists. The
current workflow for adding capability to the worker side is:

1. **Decide where the work belongs.** Pure CHAT/AST work belongs in
   the sibling `batchalign` crate (Rust), not in `batchalign-pyo3`.
   Only ML inference and provider-side glue go in the PyO3 layer.
2. If the work is ML inference, add the executor in the appropriate
   `worker_*_exec.rs` file under `crates/batchalign-pyo3/src/` and
   wire it into the IPC dispatch.
3. Add Rust tests in the same crate.
4. If you changed shared types in `batchalign-types`, regenerate any
   IPC type mirrors.
5. Rebuild `batchalign_core` (`just batchalign develop`, or just `uv run …` for incremental dev rebuilds).
6. Test against real corpus data, not just unit tests.

If you find yourself wanting to call the new function from Python
"directly" rather than through the worker IPC dispatch, that's a
sign the work belongs in the Rust runtime instead.
