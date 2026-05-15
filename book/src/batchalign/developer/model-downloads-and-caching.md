# Model Downloads and Caching (Developer Reference)

**Status:** Current
**Last updated:** 2026-05-06 13:34 EDT

This page documents how batchalign3 downloads, caches, and verifies ML
models — the contributor-facing complement to the
[user-facing chapter](../user-guide/model-downloads.md). It is the
authoritative inventory of every model load site, every cache location, and
every download mechanism BA3 currently uses.

## The on-demand download contract

batchalign3 has one rule for ML models: **download on demand, transparently,
or surface a real error.** Concretely:

1. Every model family auto-downloads on first use through the upstream
   library's standard mechanism (Stanza's `DownloadMethod.REUSE_RESOURCES`,
   HuggingFace's `from_pretrained()`, torchaudio's `pipelines.MMS_FA.get_model()`).
2. No code in BA3 may opt out of these defaults. Specifically banned:
   `local_files_only=True`, `HF_HUB_OFFLINE` / `TRANSFORMERS_OFFLINE`
   forced in BA3-controlled environment, `DownloadMethod.NONE`, or
   pre-flight existence checks that reject before the library would
   download. (One regression-gate test enforces this; see below.)
3. Any download that would block the worker for more than a second emits a
   `progress_v2` event with user-facing wording, propagated to every UI
   surface. See the [time transparency principle](../architecture/time-transparency.md).
4. A real failure (network, disk, auth) surfaces as a typed error the
   orchestrator can classify and the user can act on. Silent return-None
   on failure is the bug pattern this contract was written to prevent.

This contract was made explicit on 2026-05-06 after a fresh-install code
path silently failed: BA3 swallowed Stanza's `ResourcesFileNotFoundError`,
returned `None` from `get_cached_capability_table()`, and the Stanza
pre-flight gate translated the silent-None into "language not supported"
— misleading for a user whose Stanza catalog had simply never been
seeded. A single-host instance of that loop (orchestrator retry × worker
exit-1 × full Python traceback) generated multi-GB of `server.log` spam
per day.

## Inventory: every model load site

Source verified by reading code on 2026-05-06.

| # | Family | Load site | Library | Cache root |
|---|---|---|---|---|
| 1 | Stanza morphosyntax | `python/batchalign/worker/_stanza_loading.py:99` `load_stanza_models` | `stanza.Pipeline(download_method=REUSE_RESOURCES)` | Stanza `DEFAULT_MODEL_DIR` |
| 2 | Stanza utseg | `_stanza_loading.py:280` `load_utseg_builder` | (same) | (same) |
| 3 | Stanza Chinese retok | `_stanza_loading.py:235` `load_stanza_retokenize_model` | (same) | (same) |
| 4 | Stanza coref (lazy) | `python/batchalign/inference/coref.py:66-68` | `stanza.Pipeline(...)` | (same) |
| 5 | Whisper ASR | `python/batchalign/inference/asr.py:119` `load_whisper_asr` | `transformers.pipeline + WhisperProcessor.from_pretrained` | HF |
| 6 | Whisper FA | `python/batchalign/inference/fa.py:114` `load_whisper_fa` | `WhisperForConditionalGeneration.from_pretrained` + `WhisperProcessor.from_pretrained` | HF |
| 7 | Wave2Vec FA | `python/batchalign/inference/fa.py:198` `load_wave2vec_fa` | `torchaudio.pipelines.MMS_FA.get_model()` | torchaudio hub |
| 8 | Cantonese FA | `python/batchalign/inference/languages/cantonese/_cantonese_fa.py` `load_cantonese_fa` | `Wav2Vec2ForCTC.from_pretrained` | HF |
| 9 | SeamlessM4T translation | `python/batchalign/worker/_model_loading/translation.py:40-56` | `AutoProcessor.from_pretrained` + `SeamlessM4TModel.from_pretrained` | HF |
| 10 | pyannote diarization | `python/batchalign/inference/speaker.py:350` | `Pipeline.from_pretrained("talkbank/dia-fork")` | HF |
| 11 | NeMo speaker (fallback) | `python/batchalign/inference/speaker.py` (NeMo branch) | `EncDecSpeakerLabelModel.from_pretrained(...)` | NeMo cache |
| 12 | BERT utterance | `python/batchalign/models/utterance/infer.py:120-128` | `AutoTokenizer.from_pretrained` + `BertForTokenClassification.from_pretrained` | HF |
| 13 | PyCantonese | (bundled) | — | (none — wheel) |

Cache roots resolve to OS-specific paths via each library's own logic. See
the [user-facing chapter](../user-guide/model-downloads.md) for the table
of OS-resolved paths.

### Stanza `DEFAULT_MODEL_DIR` (1.11+)

Resolves via `os.getenv('STANZA_RESOURCES_DIR', os.path.join(USER_CACHE_DIR, 'resources'))`
in `stanza/resources/common.py:38-41`. `USER_CACHE_DIR` is the platform
cache plus a versioned subdirectory:

- macOS: `~/Library/Caches/stanza/<resver>/resources/`
- Linux: `~/.cache/stanza/<resver>/resources/`
- Windows: `%LocalAppData%\stanza\<resver>\resources\`

The historical `~/stanza_resources/` from older Stanza versions is no
longer used and any references to it in BA3 docs are bugs to fix.
`<resver>` is the resource-format version (e.g., `1.11.0`), independent of
the package version (e.g., `1.11.1`).

### HuggingFace cache resolution (current)

Order: `HF_HUB_CACHE` env > `HF_HOME` env > default
(`~/.cache/huggingface/hub` on Unix, `%LocalAppData%\huggingface\hub` on
Windows). The legacy `TRANSFORMERS_CACHE` is no longer consulted by
current `huggingface_hub`; do not reintroduce it.

## Catalog bootstrap (Stanza-specific)

Stanza ships its package code without `resources.json`. The catalog must be
downloaded once before any language pack can be resolved. BA3 does this
automatically:

- `_stanza_capabilities.py:get_cached_capability_table()` calls
  `build_stanza_capability_table()`, which calls
  `stanza.resources.common.load_resources_json()`.
- On `ResourcesFileNotFoundError` (a subclass of `FileNotFoundError`),
  `_bootstrap_and_retry()` calls `stanza.resources.common.download_resources_json()`,
  emits start/complete `progress_v2` events, and rebuilds the table.
- A real download failure raises typed `StanzaCatalogDownloadError`; the
  orchestrator should classify this as non-retryable at the worker-
  bootstrap layer (filed separately).
- `ImportError` on `import stanza` is the one legitimate silent-None path:
  it means the worker venv lacks the package, which is a deploy-config
  error, not a recoverable miss.

The pre-flight capability table itself remains the right thing for
rejecting languages Stanza does not actually have processors for (e.g.
`que`). It MUST NOT block on missing-but-downloadable resources. That
distinction is what the catalog bootstrap exists to enforce.

## User-visible download notifications

Every download site emits a `progress_v2` event so the user sees what's
happening. The shared helper lives at
`python/batchalign/worker/_progress.py`:

- `emit_download_event(stage, user_message, request_id=None, size_bytes_estimate=None)`
  — generic, used for non-HF downloads (Stanza catalog, Stanza language
  packs, torchaudio bundles).
- `emit_hf_download_if_missing(model_id, kind, request_id=None)` — probes
  the HuggingFace cache via `try_to_load_from_cache`; emits only when the
  model is genuinely about to download. Wraps every `from_pretrained()`
  call.

Sample: every HF load site looks like

```python
from batchalign.worker._progress import emit_hf_download_if_missing

emit_hf_download_if_missing("openai/whisper-large-v3", kind="ASR")
pipe = pipeline("automatic-speech-recognition", model="openai/whisper-large-v3", ...)
```

The wrapping is cheap (one cache probe), idempotent for cached models
(probe returns hit, no event emitted), and safe under failure (probe
exceptions log debug-level and emit anyway — a false-positive notification
is a much smaller UX cost than a silent multi-minute wait).

User-message wording must convey four things: what's downloading, the
approximate size, that it's a one-time cost, and that future runs will be
instant. Size hints for the largest models are tabulated in
`_progress.py` `_HF_SIZE_HINTS_GB`; expand the table when adding new
families.

## Audit gates (regression prevention)

A static check in `batchalign/tests/test_progress_audit.py` (planned)
asserts that no new code reintroduces opt-outs. Specifically, it greps for:

- `local_files_only=True` in any `from_pretrained()` call
- `HF_HUB_OFFLINE` or `TRANSFORMERS_OFFLINE` set inside BA3-controlled
  environment construction (test environments may set them externally,
  which is fine)
- `DownloadMethod.NONE` in any Stanza `Pipeline()` call
- pre-flight existence checks that raise before the library would download

If any future PR needs an exception (e.g., an offline-test fixture), it
must be opt-in via a code-path-specific flag, not a default.

## Pipeline-result caching (orthogonal to model caching)

batchalign3 caches **per-utterance audio-task results** in a tiered cache
so repeated `align` or `transcribe` runs do not redo expensive ASR / FA
passes. This is unrelated to ML-model caching: result-cache invalidation
follows the pipeline version, not the model file.

| Layer | Storage | TTL | Location |
|---|---|---|---|
| Hot | `moka` in-memory | Per-process lifetime | RAM |
| Cold | SQLite | Persistent | `~/.local/share/batchalign3/cache.db` (Linux), `~/Library/Application Support/batchalign3/cache.db` (macOS), `%LocalAppData%\batchalign3\cache.db` (Windows) |

Cached task kinds are enumerated in
`crates/batchalign/src/chat_ops/cache_key.rs::CacheTaskName`. Cache keys
include:

- Pipeline version (bumped on algorithm changes — see change log below)
- Language code
- Engine version
- Relevant per-task inputs

Text NLP tasks (`morphotag`, `utseg`, `translate`, `coref`) are NOT
cached — running them twice runs the model twice.

### Cache-breaking changes log

When the morphosyntax pipeline changes in a way that produces different
results for the same input, the pipeline version constant in `cache.rs`
(`MORPHOSYNTAX_PIPELINE_VERSION`) is bumped. This invalidates all old
cached results automatically — no user action required.

| Version | Date | Change | Impact |
|---|---|---|---|
| 1 | pre-2026 | Original Stanza-only pipeline | Baseline |
| 2 | 2026-03-23 | Added PyCantonese POS override for Cantonese (`yue`) | All cached Cantonese %mor results invalidated. Re-running morphotag on Cantonese files produces corrected POS tags (佢哋→PRON instead of PROPN, etc.). Non-Cantonese cache entries unaffected but also invalidated (harmless — recomputed with same results). |

When to bump:
- Adding or removing POS post-processing (e.g., PyCantonese override)
- Changing UD→CHAT mapping rules in `nlp/`
- Changing Stanza model selection for a language
- Fixing a bug that changes %mor / %gra output for existing inputs

When NOT to bump:
- Adding a new language (new cache keys, no collision)
- Changing cache storage implementation (keys unchanged)
- Fixing a bug in cache lookup/storage logic (not content)

## Test strategy

### Unit tests (no network, no models)

Bootstrap behavior under mocked filesystem and Stanza APIs lives in
`batchalign/tests/test_stanza_capabilities.py`. The three load-bearing
cases:

- `test_bootstrap_downloads_catalog_when_missing` — `resources.json`
  absent + download succeeds → populated table returned.
- `test_bootstrap_raises_typed_error_on_download_failure` — absent +
  download fails → `StanzaCatalogDownloadError`.
- `test_stanza_not_installed_returns_none` — `ImportError` → `None`
  (unchanged silent-None path, the only legitimate one).

These run in the default `pytest` profile (no `-m golden` needed); they
mock all I/O.

### Golden tests (real models, network on first run)

Tests that load real ML models are marked `@pytest.mark.golden` and
excluded from the default `pytest` run:

```bash
uv run pytest -m golden                # Python golden tests
cargo nextest run --profile ml         # Rust ML golden tests
```

Models download automatically on first run. Subsequent runs use the cache.
First-run download is slow (minutes for Stanza, longer for Whisper).

### Fresh-install integration test

`batchalign/tests/integration/test_fresh_install_morphotag.py` (planned)
nukes the Stanza cache, starts a daemon, submits a morphotag job, asserts
success. This is the canonical regression gate for the on-demand
contract: if it fails, BA3 has reintroduced a download opt-out somewhere.

### OOM protection in golden tests

On machines with < 128 GB RAM, the `conftest.py` guard forces golden tests
to run sequentially (`-n 0`) even if the default `pytest.ini` specifies
parallel workers. Each Stanza model instance uses 2–5 GB — parallel
workers on a 64 GB machine OOM-crash. The guard cannot be bypassed; it
fires per-test inside xdist workers via an autouse fixture.

PyCantonese tests run in the default suite because PyCantonese is bundled
(no download needed) and fast (~3s for all segmentation tests).

### What to expect on first run

| Test suite | First-run download | Subsequent runs |
|---|---|---|
| `uv run pytest` (default) | PyCantonese: 0s (bundled) | < 1s |
| `uv run pytest -m golden` | Stanza English: ~2 min, Stanza Chinese: ~2 min | < 30 s |
| `cargo nextest run --profile ml` | Stanza + Whisper: ~5–10 min | < 2 min |

## Adding a new model load site

1. Identify the upstream library's auto-download API (`from_pretrained`,
   `Pipeline`, `get_model`, etc.). Use it as-is. Do not pre-flight-check.
2. Add a `progress_v2` emit immediately before the load:
   - HuggingFace: `emit_hf_download_if_missing(model_id, kind=...)`.
   - Stanza language pack: extend the helper in `_stanza_loading.py` (or
     copy its shape).
   - Other libraries: use `emit_download_event(stage, user_message)`.
3. Add a size-hint entry to `_HF_SIZE_HINTS_GB` if the model is > 100 MB,
   so the user sees a useful estimate.
4. Update the [user-facing chapter](../user-guide/model-downloads.md)
   table with the new family + size + first-run wait estimate.
5. Update this page's inventory table.
6. Add a golden-marked test that exercises a fresh download path.

## Related references

- [User-facing model-downloads chapter](../user-guide/model-downloads.md).
- [Time transparency principle](../architecture/time-transparency.md).
- The contract enforcement code: `python/batchalign/worker/_stanza_capabilities.py`,
  `python/batchalign/worker/_progress.py`, `python/batchalign/worker/_protocol.py`.
- Bootstrap regression tests: `batchalign/tests/test_stanza_capabilities.py`.
