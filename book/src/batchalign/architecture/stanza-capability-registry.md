# Stanza Capability Registry

**Status:** Current
**Last updated:** 2026-05-06 21:30 EDT

The Stanza capability registry replaces 7 scattered hardcoded language tables
with a single runtime-authoritative data structure built from Stanza's own
`resources.json`. It answers questions like "does Dutch have constituency
parsing?" and "what is the alpha-2 code for Norwegian?" without any hardcoded
assumptions about Stanza's feature matrix.

## Why This Exists: One Authority for "Can Stanza Do X For Lang Y?"

There are several places in the pipeline that need to ask the
language-capability question, in two separate dimensions:

- **What ISO-639-3 codes does Stanza support, and with which processors?**
  (e.g. does `mar` have `tokenize`/`pos`/`lemma`/`depparse`? does `nld`
  have `mwt`? does `eng` have `constituency`?)
- **For a given ISO-639-3 code, what alpha-2 catalog key does Stanza
  use?** (e.g. `mar` → `mr`, `yue` → `zh-hans`, `nor` → `nb`).

Both questions must always have the same answer at every site that
asks them. **Two answers diverging is the recurring failure mode this
registry exists to prevent.** Every drift incident has the same shape:
one component (Rust validator, Python preflight, capability exporter)
says "yes, lang X is supported"; another component (Stanza's actual
`Pipeline()` constructor, the model loader, an alpha-2 lookup) says
"no, lang X has no models." A worker dies during bootstrap, the user
sees a generic IPC error, and the linguistic root cause is buried in
stderr.

### Drift incident history

The bug pattern keeps recurring. Each entry below is a real incident
where the registry's invariant — *"every lookup of language capability
goes through the same authority"* — was violated.

| Date | What drifted | Symptom | Root cause |
|------|--------------|---------|------------|
| 2026-04-15 | `MWT_LANGS` (Python) said Swedish (`sv`) had MWT, but Stanza's catalog had dropped the Swedish MWT model. | Every Swedish worker spawn raised `UnsupportedProcessorError`; an overnight 500-file morphotag run lost an entire language batch. | Hand-edited mirror of Stanza's catalog drifted on upgrade. |
| 2026-05-06 | `iso3_to_alpha2()` (Python) had a hardcoded ISO-3 → ISO-1 mapping that did **not** include Marathi (`mar` → `mr`); the capability table built via pycountry **did** include it. The bootstrap preflight said "yes, supports Marathi", then `iso3_to_alpha2` returned `"mar"` verbatim. | Worker for `[- mar]` whole-utterance utterance crashed with `ValueError: Language mar is currently unsupported` because Stanza catalog keys are alpha-2; `mar` is not a key. The worker never emitted its ready signal; the file failed with a generic worker-bootstrap error. | A second hardcoded language dict that needed to agree with the registry but didn't. |
| (older) Dutch utseg crash | `CONSTITUENCY_LANGS` (Python utseg) included Dutch, but Stanza shipped no Dutch constituency model. | Every Dutch utseg request crashed Stanza. | Hand-edited mirror of Stanza's catalog drifted. |

The pattern across all three: a hand-edited list of "which langs have
processor X" or "which iso3 maps to which alpha-2" got out of sync
with Stanza's installed catalog or with another component's view of
the same question.

### Former hardcoded tables

Pre-registry, processor availability and code mapping were scattered
across independent tables in Python and Rust. The registry is the
single replacement for all of them.

| Former table | Location | What it hardcoded | Now |
|--------------|----------|-------------------|-----|
| `MWT_LANGS` | Python inference | Languages with multi-word token expansion | Replaced by `StanzaRegistry::has_mwt()`. |
| `SUPPORTED_STANZA_CODES` | `stanza_languages.rs` | ISO-639-3 codes Stanza could process | Hardcoded fallback only; live registry overrides. |
| `iso3_to_alpha2` (hardcoded dict) | Python worker | ISO-639-3 → ISO-639-1 code mapping for Stanza model loading | Reduced to a small `_ISO3_OVERRIDES` (Stanza-specific deviations only) shared with the capability table builder. Standard 1-to-1 cases now go through `pycountry`. |
| `CONSTITUENCY_LANGS` | Python utseg | Languages with constituency parsing | Replaced by `StanzaRegistry::has_constituency()`. |
| ~~`STANZA_SUPPORTED_ISO3`~~ (removed) | `request.rs` | Was a duplicate of `SUPPORTED_STANZA_CODES` | Removed; submission validation delegates to the chat-ops list. |
| Inline checks in `_stanza_loading.py` | Python worker | Ad hoc feature gating | Replaced by `StanzaRegistry` queries via the capability table. |
| Inline checks in `batch.rs` | Rust morphosyntax | Language filtering | Replaced by registry queries. |

### The principle (re-derive when in doubt)

1. **Stanza's installed catalog (`resources.json`) is the only source
   of truth** for which languages have which processors.
2. **`pycountry` is the only source of truth** for the standard
   ISO-639-3 ↔ ISO-639-1 mapping.
3. **`_ISO3_OVERRIDES`** in `_stanza_capabilities.py` is the only
   place to record Stanza-specific deviations from the standard
   mapping (`yue/cmn/zho → zh-hans`, `nor → nb`, etc.).
4. **Every iso3↔alpha2 conversion** in the worker imports
   `_ISO3_OVERRIDES` from that one site rather than redefining it.
   `iso3_to_alpha2` does this; do not reintroduce a second override
   dict.
5. **Every "is lang X supported" check** at submission, dispatch, or
   load time goes through the live `StanzaRegistry` (with the chat-ops
   `SUPPORTED_STANZA_CODES` list as a conservative fallback only when
   the registry has not yet been populated by the first worker).

## Architecture: Data Flow

The registry is built once at worker startup and flows from Python to Rust
through the worker capabilities IPC protocol.

```mermaid
flowchart TD
    subgraph Python["Python Worker (python/batchalign/worker/)"]
        resources["Stanza resources.json\n(installed package data)"]
        builder["build_stanza_capability_table()\n(_stanza_capabilities.py)"]
        pycountry["pycountry\n(ISO-639-3 to alpha-2)"]
        overrides["_ISO3_OVERRIDES\n(nor→nb, yue→zh-hans, etc.)"]
        table["StanzaCapabilityTable\n(dict: iso3 → StanzaLanguageCapability)"]
        handlers["handle_capabilities()\n(_handlers.py)"]
        response["CapabilitiesResponse\n(.stanza_capabilities field)"]
    end

    subgraph Rust["Rust Server (crates/batchalign/)"]
        pool["WorkerPool\n(worker/pool/mod.rs)"]
        oncelock["OnceLock&lt;StanzaRegistry&gt;\n(populated on first worker)"]
        registry["StanzaRegistry\n(stanza_registry.rs)"]
        validation["validate_language_with_registry()\n(types/request.rs)"]
        dispatch["morphosyntax batch dispatch\n(morphosyntax/batch.rs)"]
        utseg_dispatch["utseg dispatch\n(utseg.rs)"]
    end

    subgraph Fallback["Hardcoded Fallback"]
        hardcoded["SUPPORTED_STANZA_CODES\n(stanza_languages.rs)\n~50 languages"]
    end

    resources --> builder
    pycountry --> builder
    overrides --> builder
    builder --> table
    table --> handlers --> response

    response -->|"JSON-lines IPC\nWorkerCapabilities"| pool
    pool -->|"record_capabilities()\nfirst worker only"| oncelock
    oncelock --> registry

    registry --> validation
    registry --> dispatch
    registry --> utseg_dispatch

    hardcoded -.->|"used ONLY before\nfirst worker spawns"| validation
```

<!-- Verified against:
  - python/batchalign/worker/_stanza_capabilities.py (builder, overrides, pycountry)
  - python/batchalign/worker/_handlers.py (handle_capabilities, CapabilitiesResponse)
  - crates/batchalign-types/src/worker.rs (WorkerCapabilities.stanza_capabilities, StanzaLanguageProcessors)
  - crates/batchalign/src/worker/pool/mod.rs (OnceLock, record_capabilities)
  - crates/batchalign/src/stanza_registry.rs (StanzaRegistry)
  - crates/batchalign/src/types/request.rs (validate_language_with_registry, delegates to chat_ops::is_stanza_supported)
  - crates/batchalign/src/morphosyntax/batch.rs (registry queries)
-->

## Python Side: Building the Table

`python/batchalign/worker/_stanza_capabilities.py` is the single source of truth.

**`build_stanza_capability_table()`** reads `stanza.resources.common.load_resources_json()`,
which returns the full Stanza resource dictionary keyed by alpha-2 language
codes. For each language entry that has a `"tokenize"` key (filtering out
non-language entries like `"default"`), it records which processors are
available:

| Processor | Stanza key | What it enables |
|-----------|-----------|-----------------|
| `tokenize` | `"tokenize"` | Basic tokenization |
| `pos` | `"pos"` | Part-of-speech tagging |
| `lemma` | `"lemma"` | Lemmatization |
| `depparse` | `"depparse"` | Dependency parsing |
| `mwt` | `"mwt"` | Multi-word token expansion (~45 languages) |
| `constituency` | `"constituency"` | Constituency parsing (~11 languages) |
| `coref` | `"coref"` | Coreference resolution |

**ISO-639-3 mapping** is built in two passes:

1. **`_ISO3_OVERRIDES`** — a small dict of codes where pycountry is wrong or
   Stanza uses non-standard identifiers (e.g., `nor→nb`, `yue→zh-hans`,
   `cmn→zh-hans`, `zho→zh-hans`, `msa→ms`).
2. **`pycountry.languages`** — for all remaining standard mappings.

The result is a `StanzaCapabilityTable` with `languages` keyed by ISO-639-3
code and `iso3_to_alpha2` for the code mapping. This is cached with
`@functools.lru_cache(maxsize=1)` for the process lifetime.

## IPC: Capabilities Response

When the Rust server queries a worker's capabilities (`_handlers.py:handle_capabilities()`),
the handler converts the `StanzaCapabilityTable` into a
`dict[str, StanzaLanguageProcessors]` (keyed by ISO-639-3, each value
containing an `alpha2` string and a `processors` list). This is serialized
as the `stanza_capabilities` field of `CapabilitiesResponse`
(`python/batchalign/worker/_types.py`).

On the Rust side, `WorkerCapabilities` in `crates/batchalign-types/src/worker.rs`
mirrors this structure with `stanza_capabilities: BTreeMap<String, StanzaLanguageProcessors>`.

## Rust Side: StanzaRegistry

`crates/batchalign/src/stanza_registry.rs` stores the deserialized
capabilities and provides typed query methods:

| Method | What it checks | Used by |
|--------|---------------|---------|
| `supports_morphosyntax(iso3)` | tokenize + pos + lemma + depparse | Submission validation, batch dispatch |
| `has_mwt(iso3)` | `mwt` processor available | Morphosyntax pipeline (includes MWT step only when available) |
| `has_constituency(iso3)` | `constituency` processor available | Utseg dispatch (falls back to sentence-boundary without it) |
| `alpha2(iso3)` | ISO-639-3 to Stanza alpha-2 | Model loading configuration |
| `supported_languages()` | All ISO-639-3 codes | Error messages, help text |
| `is_populated()` | Non-empty registry | Fallback gating |

### Storage: `OnceLock` in WorkerPool

The registry is stored in `WorkerPool.stanza_registry: OnceLock<Box<StanzaRegistry>>`.
`record_capabilities()` populates it from the first worker that reports
non-empty `stanza_capabilities`. The `OnceLock` ensures this is a one-shot
operation even under concurrent worker spawning.

Access is via `WorkerPool::stanza_registry() -> Option<&StanzaRegistry>`.
`None` means no worker has reported yet.

### Two-Phase Validation

Submission validation runs in two phases:

1. **Pre-filter** (`validate_language_support()` in `request.rs`):
   delegates to `chat_ops::stanza_languages::is_stanza_supported`,
   which consults the hardcoded `SUPPORTED_STANZA_CODES` set (~50
   languages). This runs even before any worker has started, so it
   catches obviously unsupported languages immediately.

2. **Authoritative check** (`validate_language_with_registry()` in
   `request.rs`): uses the live `StanzaRegistry` when available. This is
   called from `materialize_submission_job()` in `routes/jobs/mod.rs`
   where the registry is accessible via `AppState`.

Once the registry is populated, it supersedes the hardcoded table. The
hardcoded table exists only as a conservative safety net for the window
between server startup and the first worker's capability report.

## Per-Command Processor Requirements

| Command | Required processors | Optional processors |
|---------|-------------------|---------------------|
| morphotag | tokenize + pos + lemma + depparse | mwt (if available) |
| utseg | tokenize + pos | constituency (degrades to sentence-boundary without it) |
| coref | English-only, uses Stanza coref | -- |
| translate | Does not use Stanza | -- |
| align/transcribe | Does not use Stanza directly | -- |

## Graceful Degradation

The registry enables graceful degradation rather than hard failures:

- **No MWT:** Morphotag works without contraction expansion. The pipeline
  simply omits the `mwt` processor from the Stanza pipeline configuration.
- **No constituency:** Utseg falls back to sentence-boundary segmentation
  (`_stanza_loading.py:UtsegConfigBuilder` checks the capability table).
- **No depparse:** Morphotag is rejected at submission time with a clear
  error message listing supported languages.
- **Unknown language:** Rejected at submission with a formatted list of
  all supported ISO-639-3 codes.

## Maintaining the Fallback Table

After upgrading Stanza, regenerate the hardcoded fallback table:

```bash
uv run scripts/generate_stanza_language_table.py
```

This reads the installed Stanza's `resources.json`. The historically
hardcoded `SUPPORTED_STANZA_CODES` constant is no longer maintained;
the authoritative table is built dynamically from the Python worker's
live Stanza resources and sent to Rust via the capability response.
Delegation paths in `request.rs` and the transcribe plan-time gate consult
the live table (with chat-ops fallback if the registry is unavailable).

The Python worker also runs a typed `UnsupportedLanguageError`
preflight in `_stanza_loading.py::load_stanza_models` that consults
the live capability table — this is the authoritative drift safety
net regardless of how stale the Rust fallback gets.

## Key Source Files

| File | Role |
|------|------|
| `python/batchalign/worker/_stanza_capabilities.py` | Reads `resources.json`, builds `StanzaCapabilityTable` |
| `python/batchalign/worker/_handlers.py` | Serializes table into `CapabilitiesResponse.stanza_capabilities` |
| `python/batchalign/worker/_types.py` | `CapabilitiesResponse`, `StanzaLanguageProcessors` Pydantic models |
| `python/batchalign/worker/_stanza_loading.py` | Utseg config builder queries table for constituency/MWT |
| `crates/batchalign-types/src/worker.rs` | `WorkerCapabilities`, `StanzaLanguageProcessors` Rust types |
| `crates/batchalign/src/stanza_registry.rs` | `StanzaRegistry` with typed query methods |
| `crates/batchalign/src/worker/pool/mod.rs` | `OnceLock<StanzaRegistry>` storage, `record_capabilities()` |
| `crates/batchalign/src/types/request.rs` | Single Rust fallback via `is_stanza_supported_language()`; delegates to chat-ops |
| `crates/batchalign/src/types/request.rs` | `validate_language_with_registry()`; submission-time gate delegates to chat-ops |
| `crates/batchalign/src/pipeline/transcribe.rs` | Plan-time gate (registry first, chat-ops fallback) |
| `python/batchalign/worker/_stanza_loading.py` | `UnsupportedLanguageError` preflight before `stanza.Pipeline` |
| `crates/batchalign/src/morphosyntax/batch.rs` | Queries registry for language filtering |
| `scripts/generate_stanza_language_table.py` | Regenerates Rust fallback tables from installed Stanza |
