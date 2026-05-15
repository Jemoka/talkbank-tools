# Adding Inference Providers

**Status:** Current
**Last updated:** 2026-05-10 12:12 EDT

Batchalign3 no longer has a public entry-point plugin system. New engines are
added in-tree as built-in worker capabilities.

This page covers the current extension path.

If you are adding a new command, choose its `WorkflowFamily` in
`crates/batchalign/src/command_family.rs`, put the released command wrapper
in `crates/batchalign/src/commands/`, and keep any algorithmic or
orchestration logic in the owning Rust module. Engine work should support that
Rust-owned command flow; it should not define the command shape on its own.

## Choose the layer first

There are two different things you might be adding:

1. A new **worker-side inference backend** such as a new ASR or FA engine.
2. A new **server command** that needs Rust-side orchestration plus, optionally,
   a new worker inference task.

Most engine work starts in Python and only touches Rust for typed IPC contracts,
command registration, and server orchestration.

## Adding a worker-side inference backend

### 1. Add the inference module

Create a built-in module under `python/batchalign/inference/` that exposes a pure
inference helper consumed by a typed V2 worker host:

```python
from __future__ import annotations

from batchalign.worker._types_v2 import MyTaskItemV2, MyTaskResultItemV2


def infer_my_task(items: list[MyTaskItemV2]) -> list[MyTaskResultItemV2]:
    results: list[MyTaskResultItemV2] = []
    for item in items:
        results.append(MyTaskResultItemV2(ok=True))
    return results
```

Keep these modules CHAT-free. Python workers should accept structured payloads
and return structured results only.

### 2. Add or reuse the task identifier

If this is a new live infer task, add it in the V2 IPC type definitions:

- `python/batchalign/worker/_types_v2.py`
- `crates/batchalign-types/src/worker_v2/` (re-exported by `crates/batchalign/src/types/worker_v2.rs`)

If you are only adding a new engine behind an existing task such as ASR or FA,
reuse the existing task and add only the new engine selector/state.

### 3. Load model state in the worker

Update `python/batchalign/worker/_model_loading/` so `load_worker_task()` can
initialize the new engine for the relevant infer task. This is where task-level
engine overrides are resolved and worker state is populated.

For existing command families, you usually update one of:

- `load_asr_engine()`
- `load_fa_engine()`
- `load_translation_engine()`
- `load_stanza_models()` in `worker/_stanza_loading.py`

### 4. Wire dispatch and capability advertisement

Update:

- `python/batchalign/worker/_execute_v2.py` to route the task or engine
- `python/batchalign/worker/_text_v2.py` if the task belongs to the shared batched
  text host
- `python/batchalign/worker/_handlers.py` to advertise `infer_tasks` and
  `engine_versions`

If the new engine is a variant of an existing task, keep the task stable and
report the engine version string through `engine_versions`.

**Capability gate (critical):** The `_capabilities()` function in `_handlers.py`
uses **import probes** to decide which infer tasks to advertise. If you add a new
`InferTask`, you must add it to the `_INFER_TASK_PROBES` dict with the tuple of
Python modules that must be importable:

```python
_INFER_TASK_PROBES: dict[InferTask, tuple[tuple[str, ...], str]] = {
    ...
    InferTask.MY_TASK: (("my_library",), "my-engine-v1"),
}
```

Capabilities are detected lazily from the first real worker spawn — there is no
dedicated probe worker at startup. The capability check uses import probes, not
loaded model state. This means capability advertisement must be based on import
availability, never on `_state.my_model is not None`. If you gate on loaded
model state, your task will not be advertised and the server will silently
exclude the command.

The Rust server cross-checks: commands whose required `InferTask` is not in the
worker's `infer_tasks` list are excluded from the server's advertised
capabilities. See [Capability Discovery](../../architecture/python-rust-boundary/python-rust-boundary.md#capability-discovery)
for the full flow.

### 5. Register dependencies

Add the engine's Python dependencies to the appropriate section in
`pyproject.toml`:

- **Core engines** (expected to work out of the box): add to `dependencies`.
  All standard commands (align, transcribe, translate, morphotag, etc.) have
  their dependencies in `dependencies` so that `uv tool install batchalign3`
  gives users everything.

- **Built-in engines with extra runtime dependencies**: add them to
  `dependencies` if they are part of the supported built-in engine surface.
  Credential-gated or region-specific does not imply a separate install tier.

  Users then install `batchalign3[my-engine]`.

### Cross-cutting Rust edits for a new ASR engine variant

For an ASR engine specifically, a variant lives in **three Rust enums
that must stay in sync** — a mismatch in any one silently mis-routes
dispatch. The following tables enumerate every file/identifier you
must update when adding a new variant. Use the `whisper_hub` addition
(2026-04-22) as a worked example of every line item.

**The three-enum synchronization:**

| Enum | File | Role |
|------|------|------|
| `AsrEngineName` | `crates/batchalign/src/types/engines.rs` | User-facing type. Wire name, parsing, dispatch-key lookup. |
| `AsrBackendV2` | `crates/batchalign-types/src/worker_v2/requests.rs` | IPC contract with Python workers. Regenerate after edit via `bash scripts/generate_ipc_types.sh` (it runs `datamodel-code-generator` on the JSON schema produced by `generate_ipc_schema()`). |
| `AsrWorkerMode` | `crates/batchalign/src/transcribe/types.rs` | Server-side dispatch selector that bridges the other two. |

**Helpers each variant must appear in:**

| Function | File | Purpose |
|---|---|---|
| `AsrEngineName::wire_name()` | `types/engines.rs` | Rust→string for JSON/SQLite. |
| `AsrEngineName::try_from_wire_name()` | `types/engines.rs` | String→Rust at boundaries. |
| `AsrEngineName::dispatch_override_name()` | `types/engines.rs` | Pool key (must equal wire_name or `None`). |
| `AsrWorkerMode::from_engine_name()` | `transcribe/types.rs` | Wire-string → worker-mode variant. |
| `AsrWorkerMode::as_v2_backend()` | `transcribe/types.rs` | Worker-mode → IPC backend. |
| `AsrBackend::comment_engine_name()` | `transcribe/types.rs` (test-only) | Canonical wire string for tests. |
| `asr_backend_override_name()` | `crates/batchalign/src/worker/pool/execute_v2.rs` | Pool-key string; must match `dispatch_override_name()`. |
| *(input-source routing)* | `crates/batchalign/src/transcribe/infer.rs` | Match on `AsrWorkerMode` picks `PreparedAudio` (local model) vs `ProviderMedia` (external service). |

Worker-side enum (matches Rust wire name one-to-one):

- `AsrEngine` in `python/batchalign/worker/_types.py` — the Python enum the
  worker bootstrap stores in `_state.asr_engine`.

Request validation surface (optional, only for engines with
per-engine language constraints like the Cantonese ASR engines):

- `validate_language_support()` in `crates/batchalign/src/types/request.rs`.

**Per-language default model_id resolution.** If your engine picks a
model per language (e.g. different HF fine-tunes per language), add
entries to `python/batchalign/models/resolve.py` rather than inventing a new
per-engine table. `resolve("your_engine", lang_iso3)` returns the
model_id or `None`; raise a typed error on `None` unless the caller
passed an explicit override, rather than falling back to a generic
default.

**HF Whisper fine-tune gotcha.** HF community Whisper fine-tunes bake
`language` and `task` into their own `generation_config`. Passing
those again in `generate_kwargs` produces gibberish. The escape hatch
is the `skip_language_force: bool` flag on
`python/batchalign/inference/types.py::WhisperASRHandle` — when `True`,
`gen_kwargs()` returns ONLY `{"max_new_tokens": 444}` and omits
`task`, `language`, `generation_config`, and `repetition_penalty`.
See `python/batchalign/inference/whisper_hub.py` for the wiring:
pass `language="auto"` to `load_whisper_asr()` AND set
`handle.skip_language_force = True` before returning.

**Why the `max_new_tokens=444` safety cap.** With empty
`generate_kwargs`, the HuggingFace ASR pipeline can let a fine-tune
fall into a non-converging decoder state where it never predicts an
end-of-utterance token, hanging the worker for tens of minutes. The
cap is a hard upper bound on tokens-per-chunk, not a probability
override, so it is a no-op on successful runs but terminates
runaways. The value 444 is one below Whisper's legal max:
`max_target_positions = 448` includes the 3 special start tokens,
leaving 445 for new tokens, with 444 chosen for one token of margin.

## TDD discipline for engine additions

The rest of this section is a TDD checklist derived from actually
shipping (and breaking) `whisper_hub`. Every item is reactive: it
corresponds to a mistake that was made once and should be prevented
by test structure in the future.

### Test at the observable boundary, not at the function you call into

When a loader or constructor populates a stateful intermediate (a
handle, a worker state, a registry entry), do not substitute tests
that assert "the right thing was passed *into* the constructor" for
tests that assert "the object the constructor returns, when exercised
by a downstream caller, produces the right observable behavior."

Concrete example. The `whisper_hub` loader was initially tested only
by asserting that `load_whisper_asr` received `language="auto"` —
a proxy for "fine-tunes won't get their language re-forced at
`generate()` time." That assertion was true but insufficient: the
V2 inference path (`infer_whisper_prepared_audio`) calls
`handle.gen_kwargs(request_lang)` and ignores `handle.lang`
entirely. The fine-tune was receiving `task="transcribe",
language="malayalam"` at every `generate()` call and would have
produced cross-script gibberish. The unit tests did not fail because
nothing ever exercised the actual runtime path.

The fix was an additional test that constructs a
`WhisperASRHandle(skip_language_force=True)` directly, calls
`gen_kwargs("malayalam")` on it, and asserts `task` and `language`
are absent from the returned dict. That test exercises the runtime
contract that production depends on.

General rule: **for every stateful intermediate in the pipeline,
there must be tests on both sides of it.** Input-side tests verify
the construction call site. Output-side tests verify that downstream
callers, given only the constructed object, see the right behavior.

### Grep for every method that consumes the state you set

When you set a field on a shared handle or state object, grep for
every call site that reads that field OR reads a seemingly-unrelated
field that could diverge. `gen_kwargs(lang)` reading
*the caller-supplied* `lang` rather than `self.lang` is exactly that
kind of divergence: two plausibly-interchangeable data sources where
only one was the contract.

```bash
# What reads self.lang?
rg 'model\.lang|handle\.lang|\.lang =' python/batchalign/inference/
# What calls gen_kwargs?
rg 'gen_kwargs\(' batchalign/
```

If the same conceptual value (the "language to transcribe in") flows
through both paths, your engine-addition test must cover both.

### Add a failing runtime-behavior test before writing any loader code

For ASR engine additions, the RED test baseline is:

1. `test_<engine>_wire_roundtrip` — `AsrEngineName::<Engine>.wire_name()`
   roundtrips through `try_from_wire_name`. Already a common idiom;
   add the variant to the existing test module.
2. `test_<engine>_worker_mode_lowers_correctly` — `AsrWorkerMode`
   variant lowers to the right `AsrBackendV2` and back.
3. `test_<engine>_loader_dispatch` — worker bootstrap's
   `load_asr_engine()` routes `engine_overrides["asr"]=="<engine>"`
   to your new loader function.
4. **`test_<engine>_handle_gen_kwargs_for_concrete_language`** —
   construct a handle the way your loader would, call its
   generation-kwargs method with a concrete (non-auto) language, and
   assert the output dict matches what you expect `generate()` to
   receive. This is the test that catches the fine-tune trap above.
5. **`test_<engine>_resolves_model_id_for_seeded_language`** — if
   your engine uses per-language defaults from `resolve.py`, pin the
   seed entry with a direct assertion on `resolve("<engine>", lang)`.
6. **`test_<engine>_raises_on_unseeded_language`** — if your engine
   raises on a missing default, pin the error type and message
   fragment. Don't let the error degrade into a silent stock
   fallback.

Guard-rail tests must accompany any deny-list / recommendation
changes — if you redirect users from engine X to engine Y for some
language, engine Y must itself pass validation for that language.

### Rebuild the PyO3 extension — the Python worker's dispatch is Rust

`AsrBackendV2` exists in two Rust crates (`batchalign-types` for the
server, and `crates/batchalign-pyo3/src/worker_asr_exec.rs` via that crate). The PyO3
function `batchalign_core.execute_asr_request_v2(request, ...)` owns
the runtime dispatch: it pattern-matches on `AsrBackendV2` inside the
worker process and routes to the right runner. Adding a new enum
variant means you must:

1. Add a match arm inside `crates/batchalign-pyo3/src/worker_asr_exec.rs::run_asr` that
   routes the new variant. The compiler will catch the missing arm if
   you let it — but only after you rebuild the PyO3 extension.
2. Rebuild `batchalign_core`: `just batchalign develop` (which
   produces a fresh wheel via the maturin backend declared in
   `pyproject.toml` and reinstalls it into the dev environment), or run
   any `uv run …` command to trigger an incremental rebuild against
   `[tool.maturin] profile = "dev"`. Running `bazel build //...` alone (which
   does `cargo build --workspace --release`) compiles the PyO3 crate but
   does **not** install the resulting `.so` into the Python environment
   that workers import from. Without the install step, workers silently
   load the previously installed `batchalign_core` extension, which has
   no match arm for your new variant and drops the request with no
   response (the Rust server then sits in its request-timeout wait for
   ~30 minutes). Neither end logs the Pydantic / serde validation
   failure that would localize this.
3. Also update the Python hand-maintained `AsrBackendV2` enum in
   `python/batchalign/worker/_types_v2.py`. The generated file at
   `python/batchalign/generated/worker_v2/AsrBackendV2.py` already gets the
   variant after `make generate-ipc-types`, but the hand-maintained
   file is what type-checks in worker source code and must be kept in
   sync by hand.

**Same class of bug as the `gen_kwargs` trap.** Both are "state
crosses a boundary I didn't search for, so the test suite never
exercises the real runtime path." The fix is the same grep ritual:

```bash
# Before declaring a new ASR variant done:
rg "AsrBackendV2::" crates/
rg 'AsrBackendV2\.'  batchalign/
```

Every match site gets a new arm. If you can't find the grep result
again on first read, the variant has a hole.

### Compile-time vs install-time mismatch

`batchalign-pyo3` is a member of the root Cargo workspace (see
`members` in the root `Cargo.toml`), so `cargo check --workspace`
and `bazel build //...` *do* compile the PyO3 bridge and *do*
exhaustive-match-check `AsrBackendV2` arms there. Targeted
commands like `cargo check -p batchalign-cli` skip it because
`batchalign` does not depend on `batchalign-pyo3`.

The failure mode the rebuild ritual prevents is therefore not a
missed compile error: it is an install gap. The freshly compiled
PyO3 `.so` lives in `target/`, while the Python worker process
imports `batchalign_core` from the wheel previously installed into
the active `uv` environment. Without `just batchalign develop`
(or an equivalent reinstall), the worker keeps loading the stale
`.so` and the new variant produces a silent stall on the worker's
stdin readline (no response surfaces until the 30-minute
audio-task timeout fires).

The grep-and-rebuild ritual above is the defense; the workspace
structure no longer is the gap.

(An out-of-date doc-comment in `crates/batchalign-pyo3/Cargo.toml`
describes the crate as "outside the root workspace by design" —
that comment predates the move into the workspace and should be
updated when next touched.)

### Structural opportunity: `gen_kwargs` takes a string

`WhisperASRHandle.gen_kwargs(lang)` dispatches on a string: `"auto"`
is one special case, `"Cantonese"` is another, and any other string
means "force this language on `generate()`". Plus the new
`skip_language_force` flag adds a fourth behavior. This is
boolean-blindness dressed in string clothing — four distinct
generation modes hidden in two orthogonal inputs. A future refactor
should replace this with an enum such as `WhisperGenMode::{
AutoDetect, FinetunePinnedByConfig, CantoneseSpecialCase,
ForceLanguage(Name) }`, with each engine's loader picking the
variant explicitly. Out of scope for a single-engine addition, but
worth doing before the *next* whisper-family engine (WhisperX,
WhisperOai Hub variant, etc.) repeats the same mistake.

## Adding a new server command

If you are adding a new top-level command (not just a new engine for an
existing command), see the detailed 8-step checklist in
[Rust CLI and Server](rust-cli-and-server.md#adding-a-new-cli-command).

In addition to those Rust-side changes, update these Python-side surfaces:

1. **`crates/batchalign-types/src/command_spec.rs`** — Add a `CommandSpec` entry to
   `COMMAND_SPECS`. Then run `cargo xtask gen-runtime-toml` to regenerate
   `python/batchalign/runtime_constants.toml` (the generated file is the shared
   Rust/Python source of truth; do not edit it directly).
2. **`batchalign/runtime.py`** — Add the command to `COMMAND_PROBES` with the
   tuple of Python modules that must be importable for the command to appear in
   `detect_capabilities()`.
3. **`python/batchalign/worker/_handlers.py`** — Add the `InferTask` to
   `_INFER_TASK_PROBES` so the worker advertises it. This must match the
   same dependencies used in `COMMAND_PROBES`. See
   [step 4 above](#4-wire-dispatch-and-capability-advertisement) for details.
4. **`python/batchalign/worker/_model_loading/`** — Register the dynamic
   runtime host for the new task if it depends on loaded model state or
   engine-specific wiring. Reserve **`python/batchalign/worker/_execute_v2.py`** for
   the small task router that dispatches to those prepared hosts.

Remember: command semantics live in the command-owned Rust layer, not in the
worker bootstrap layer. The worker layer should only know how to load engines
and execute typed tasks.

## No public extension surface

There is no public Python extension surface. New engines are added
in-tree, through the steps above; there is no `batchalign.plugins`
discovery API, no `PluginDescriptor` contract, and no supported
external-package path for adding ASR / FA / morphosyntax / etc.
backends without modifying this repo.

The worker-side modules under `batchalign.worker.*` and the
`batchalign.providers` re-export module are internal implementation
detail and may change without notice.

If you want to ship an engine without making it a mandatory
dependency of the package, use an `[project.optional-dependencies]`
extra in `pyproject.toml` so the dependency installs only when
explicitly requested.

## Test expectations

At minimum, add:

- unit tests for the new inference module
- worker dispatch tests covering `_execute_v2()` or the relevant task host
- bootstrap/handler-registration tests if the task uses dynamic worker runtime
- Rust integration coverage if the new engine changes server orchestration,
  command routing, or capability gating
- doc updates for install syntax, command options, and migration notes if this
  replaces a BA2 or pre-release workflow

Relevant existing coverage lives in:

- `batchalign/tests/` (Python-side worker dispatch / handler tests)
- `crates/batchalign/tests/` (Rust-side server / IPC / CI hygiene tests)
- inline Rust tests under `crates/batchalign-pyo3/src/` for the PyO3 bridge

## Rule of thumb

If the change affects CHAT structure, it belongs in Rust.

If the change affects model inference only, it usually belongs in Python plus
the typed worker contract that Rust consumes.
