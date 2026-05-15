# CLI Option Wiring

**Status:** Current
**Last updated:** 2026-03-17

This document maps every CLI option to its downstream consumer in the dispatch
layer. It serves as the authoritative reference for whether a flag is
actually wired to production code.

## Architecture: Two Option Paths

CLI options flow through **two separate paths** to reach production code. Both must
be audited when adding or reviewing flags.

### Path 1: Per-Command Options (`CommandOptions`)

```text
CLI args (clap) → CommandOptions (typed structs) → Dispatch extraction → Dispatch plan → Production code
      ↑                    ↑                              ↑                  ↑
  commands.rs         options.rs                    dispatch/options.rs  dispatch/plan.rs
  (parsing)        (build_typed_options)         (extract_*_dispatch_params)
```

1. **CLI parsing** (`crates/batchalign/src/args/commands.rs`) — clap structs
    define flags with defaults.
2. **Options bridge** (`crates/batchalign/src/args/options.rs`) —
    `build_typed_options()` converts parsed args into `CommandOptions` enum variants.
3. **Dispatch extraction** (`crates/batchalign/src/runner/dispatch/options.rs`) —
    pure functions extract typed params from `CommandOptions` for each dispatch path.
4. **Dispatch plan build** (`crates/batchalign/src/runner/dispatch/plan.rs`) —
   combines those extracted params with runner-only state (for example
   `runtime_state` toggles) once before the async dispatch modules run.

The extraction functions are covered by tests in `options.rs::tests` that verify
every field is read. Adding a new field to a `CommandOptions` struct without
adding a corresponding extraction test is a code review signal. The plan builders
in `plan.rs::tests` then verify that the runner-facing command families consume
those extracted parameters through a typed seam instead of re-reading
`CommandOptions` directly inside async dispatch code.

### Path 2: Job-Level Options (`Job` fields)

```text
CLI args (clap) → dispatch/paths.rs resolution → JobSubmission → Job struct → Dispatch reads Job directly
      ↑                                                ↑                            ↑
  CommonOpts                                       api.rs                    fa_pipeline.rs (job.lang, etc.)
```

Some options bypass `CommandOptions` entirely and are stored directly on the `Job`
struct. These are cross-cutting concerns shared by all commands:

| CLI Flag | `CommonOpts` Field | `Job` Field | Dispatch Consumer | Status |
|---|---|---|---|---|
| `--lang` | (positional/global) | `job.lang` | Worker pool key, worker CLI | Wired |
| `-n` / `--num-speakers` | (global) | `job.num_speakers` | `dispatch_transcribe_infer` | Wired |
| `--before PATH` | `CommonOpts.before` | `job.before_paths` | `dispatch_fa_infer`, `dispatch_batched_infer` → incremental orchestrators | Wired (morphotag, align) |
| `--in-place` | `CommonOpts.in_place` | Resolved at CLI dispatch | Path resolution in `dispatch/paths.rs` | Wired |
| `--file-list FILE` | `CommonOpts.file_list` | Resolved at CLI dispatch | File discovery in `resolve.rs` | Wired |
| `--override-media-cache` | `CommonOpts.override_media_cache` | `CommonOptions.override_media_cache` | Via `CommandOptions` (Path 1) | Wired |
| `--engine-overrides JSON` | (global) | `CommonOptions.engine_overrides` | Typed built-in/custom engine selection and worker command overrides | Wired |
| `--lexicon FILE` | (global) | `CommonOptions.mwt` | MWT dictionary injection | Wired |

**Why two paths?** Per-command options (FA engine, pauses, retokenize) are
command-specific and vary per `CommandOptions` variant. Job-level options (language,
speaker count, before paths, file discovery) are shared infrastructure that applies
across all commands.

**Audit discipline:** When adding a new CLI flag, determine which path it belongs to.
If it's per-command, add to the relevant `CommandOptions` variant AND add an extraction
test. If it's job-level, document it in the table above.

## `align`

| CLI Flag | CommandOptions Field | Dispatch Consumer | Status |
|---|---|---|---|
| `--fa-engine {wav2vec,whisper}` | `AlignOptions.fa_engine` | `extract_fa_dispatch_params` → `FaDispatchPlan` → `FaParams.engine` | Wired |
| `--fa-engine-custom NAME` | `AlignOptions.fa_engine` | Same as above | Wired |
| `--utr` / `--no-utr` | `AlignOptions.utr_engine` | `extract_fa_dispatch_params` → `FaDispatchPlan.options.utr_engine` → UTR pre-pass in `process_one_fa_file` | Wired |
| `--utr-engine {rev,whisper}` | `AlignOptions.utr_engine` | Same | Wired |
| `--utr-strategy {auto,global,two-pass}` | `AlignOptions.utr_overlap_strategy` | `extract_fa_dispatch_params` → `FaDispatchPlan.options.utr_overlap_strategy` → `run_utr_pass()` strategy resolver | Wired |
| `--pauses` | `AlignOptions.pauses` | `extract_fa_dispatch_params` → `FaDispatchPlan.options.fa_params.timing_mode` | Wired |
| `--wor` / `--nowor` | `AlignOptions.wor` | `extract_fa_dispatch_params` → `FaDispatchPlan.options.fa_params.wor_tier` | Wired |
| `--merge-abbrev` | `AlignOptions.merge_abbrev` | `extract_fa_dispatch_params` → `FaDispatchPlan.options.merge_abbrev` | Wired |
| `--before PATH` | `CommonOpts.before` (Path 2) | `job.before_paths` → `process_fa_incremental()` | Wired — copies stable `%wor` from the before file, refreshes preserved groups, and only re-aligns remaining groups |

### UTR behavior (implemented)

UTR is wired as a **detect-and-skip pre-pass** in `process_one_fa_file`
(`crates/batchalign/src/runner/dispatch/fa_pipeline.rs`), with three optimizations:
ASR result caching, fallback UTR after FA failure, and partial-window ASR for
mostly-timed files.

The core logic is in `run_utr_pass()` (shared by both the pre-pass and fallback):

1. Parse CHAT, call `count_utterance_timing()` → `(timed, untimed)`.
2. If `untimed == 0`: skip UTR → proceed to FA.
3. If `untimed > 0` AND `utr_engine.is_some()`: call `run_utr_pass()`:
   - **ASR caching:** Check the analysis cache for a prior ASR result (key =
     `BLAKE3("utr_asr|{audio_identity}|{lang}")`). On hit, skip inference
     entirely. On miss, run the selected Rust-owned UTR backend and cache the
     result.
   - **Partial-window mode:** When the file is mostly timed (`untimed_ratio < 0.5`
     and audio > 60s), `find_untimed_windows()` identifies time windows covering
     only the untimed regions. Worker-backed engines run on each extracted
     segment instead of the full file. Each segment's result is cached
     independently.
   - **Full-file mode:** For mostly-untimed files, short audio, or Rust-owned
     Rev.AI UTR, timing recovery runs on the full audio.
   - Convert ASR tokens to `AsrTimingToken`, resolve the configured overlap
     strategy (`auto`, `global`, or `two-pass`), inject timing, and re-serialize.
4. If `untimed > 0` AND `utr_engine.is_none()`: log warning, proceed to FA
   with interpolation fallback.
5. **Fallback UTR:** If FA fails with a retryable error and untimed utterances
   exist that were not recovered (UTR failed/skipped), the retry handler calls
   `run_utr_pass()` once before the next retry attempt. The `utr_fallback_attempted`
   flag ensures at most one extra ASR call across all retries.

This is **smarter than ba2's skip logic**: ba2 skipped UTR if *any* utterance
had timing (potentially missing untimed ones). ba3 skips only if *all* are timed.

The UTR module (`fa/utr.rs`) injects utterance-level bullets only — it does not
set word-level timing. FA handles word-level alignment after UTR provides the
utterance boundaries.

Without UTR, untimed utterances fall back to interpolation between neighboring
timed utterances (see [Proportional FA Estimation](../architecture/alignment/proportional-fa-estimation.md)).
This achieves ~96% coverage vs 100% with UTR.

## `transcribe`

| CLI Flag | CommandOptions Field | Dispatch Consumer | Status |
|---|---|---|---|
| `--asr-engine {rev,whisper,...}` | `TranscribeOptions.asr_engine` | `extract_transcribe_dispatch_params` → `TranscribeDispatchPlan.base_options.backend` | Wired |
| `--diarization` / `--diarize` | `TranscribeOptions.diarize` | CLI routes to `Transcribe` vs `TranscribeS` command variant, then `extract_transcribe_dispatch_params` → `TranscribeDispatchPlan.base_options.diarize` | Wired |
| `--wor` / `--nowor` | `TranscribeOptions.wor` | `extract_transcribe_dispatch_params` → `TranscribeDispatchPlan.base_options.write_wor` | Wired — gates `%wor` tier generation in `build_chat()`. Default omit (BA2 parity). |
| `--batch-size N` | `TranscribeOptions.batch_size` | `extract_transcribe_dispatch_params` → `TranscribeDispatchParams.batch_size` | **Extracted but not consumed** — not in BA2, hidden from `--help` |
| `--merge-abbrev` | `TranscribeOptions.merge_abbrev` | `extract_transcribe_dispatch_params` → `TranscribeDispatchPlan.should_merge_abbrev` | Wired |
| `--lang` | `Job.lang` | Worker pool key + worker CLI | Wired |
| `-n` / `--num-speakers` | `Job.num_speakers` | `dispatch_transcribe_infer` | Wired |

### Notes

`--asr-engine` was not a flag in batchalign2. BA2 used mutually exclusive
`--whisper/--rev/--whisperx/--whisper_oai` flags, which BA3 preserves as
compatibility aliases. The CLI now rejects invalid `--engine-overrides` JSON at
parse time, and the resolved `asr` / `fa` selections flow through typed
built-in-or-custom engine selectors before dispatch chooses the Rust-owned
Rev.AI path versus the worker-backed ASR backends.

`--batch-size` was not a flag in batchalign2 (hardcoded to 8 for WhisperX).

## `morphotag`

| CLI Flag | CommandOptions Field | Dispatch Consumer | Status |
|---|---|---|---|
| `--retokenize` / `--keeptokens` | `MorphotagOptions.retokenize` | `extract_morphotag_dispatch_params` → `BatchedInferDispatchPlan.tokenization_mode` | Wired |
| `--skipmultilang` / `--multilang` | `MorphotagOptions.skipmultilang` | `extract_morphotag_dispatch_params` → `BatchedInferDispatchPlan.multilingual_policy` | Wired |
| `--lexicon FILE` | `CommonOptions.mwt` | Loaded at CLI, injected into `CommonOptions.mwt` | Wired |
| `--merge-abbrev` | `MorphotagOptions.merge_abbrev` | `extract_morphotag_dispatch_params` → `BatchedInferDispatchPlan.should_merge_abbrev` | Wired |
| `--before PATH` | `CommonOpts.before` (Path 2) | `job.before_paths` → `process_morphosyntax_incremental()` | Wired — skips NLP for unchanged utterances |

## `benchmark`

| CLI Flag | CommandOptions Field | Dispatch Consumer | Status |
|---|---|---|---|
| `--asr-engine` | `BenchmarkOptions.asr_engine` | `extract_benchmark_dispatch_params` → `BenchmarkDispatchPlan.base_options.backend` | Wired |
| `--wor` / `--nowor` | `BenchmarkOptions.wor` | `extract_benchmark_dispatch_params` → `BenchmarkDispatchPlan.base_options.write_wor` | Wired — gates `%wor` tier generation on the benchmark's transcription output. Default omit (BA2 parity). |
| `--merge-abbrev` | `BenchmarkOptions.merge_abbrev` | `extract_benchmark_dispatch_params` → `BenchmarkDispatchPlan.should_merge_abbrev` | Wired |

## `opensmile`

| CLI Flag | CommandOptions Field | Dispatch Consumer | Status |
|---|---|---|---|
| `--feature-set SET` | `OpensmileOptions.feature_set` | `extract_opensmile_dispatch_params` → `MediaAnalysisDispatchPlan::Opensmile` → `dispatch_opensmile_attempt` | Wired |

## `translate`, `coref`, `utseg`, `compare`

These commands have only `--merge-abbrev` as a command-specific option, which is
wired through `BatchedInferDispatchPlan.should_merge_abbrev`.

## `avqi`

No command-specific options (only global options).

## Testing

### Path 1: CommandOptions extraction tests

The extraction functions in `crates/batchalign/src/runner/dispatch/options.rs`
have comprehensive tests that exercise every field of every `CommandOptions` variant
with non-default values, and `plan.rs` verifies the runner-facing command-family
plans consume those extracted values. Run with:

```bash
cargo nextest run -p batchalign-cli -E 'test(runner::dispatch::options)'
cargo nextest run -p batchalign-cli -E 'test(runner::dispatch::plan)'
```

### Path 2: Job-level options

Job-level options (`before_paths`, `lang`, `num_speakers`) are tested via:

- **Diff engine tests** (`batchalign`): 11 tests covering `diff_chat()` classification
- **Incremental orchestrator tests**: `process_fa_incremental` and
  `process_morphosyntax_incremental` are integration-tested via worker tests
- **Path resolution tests**: `dispatch/paths.rs` `--before` directory/file matching

```bash
cargo nextest run -p batchalign-cli -E 'test(diff)'
cargo nextest run -p batchalign-cli -E 'test(job_level_options)'
```

### UTR testing

```bash
# Unit tests for UTR (timing injection, cache keys, window finder)
cargo nextest run -p batchalign-cli -E 'test(fa::utr)'

# Cache key stability tests
cargo nextest run -p batchalign-cli -E 'test(cache_key)'

# All FA tests (UTR + grouping + alignment + injection)
cargo nextest run -p batchalign-cli -E 'test(fa::)'
```

---
Last updated: 2026-03-16
