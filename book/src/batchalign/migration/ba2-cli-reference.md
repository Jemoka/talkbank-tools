# Batchalign2 CLI Reference (Baseline)

**Status:** Reference
**Last updated:** 2026-06-01 21:36 PDT

This document captures the CLI surface of Batchalign2 across **two baselines**:

- **Jan 9 baseline** (`84ad500b`): the primary migration anchor.
- **Feb 9 master** (`e8f8bfad`): the later released BA2 master branch,
  which added concurrency, caching, `compare`, `cache`, `bench`, and
  several global options not present in the Jan 9 baseline.

Features that exist only in the Feb 9 master (not in the Jan 9 baseline) are
marked with **(Feb 9 only)** throughout this document. The Jan 9 baseline CLI
was simpler: only `-v/--verbose` as a global option, and no `compare`, `cache`,
or `bench` commands.

**Source:** `batchalign/cli/cli.py` using `rich_click` (Click wrapper).

---

## Global Options

These are defined on the top-level `batchalign` group and available to all
commands.

**Jan 9 baseline:** The only global option was `-v/--verbose`. All other global
options listed below were added in the Feb 9 master.

The Feb 9 globals BA3 carries forward (as wired flags) are
`--verbose`, `--workers`, `--timeout`, and `--force-cpu`. Every other
Feb 9 global was removed — passing it to BA3 produces a clap parse
error, not a silent no-op.

| Flag | Type | Default | Baseline | BA3 Status |
|------|------|---------|----------|------------|
| `-v` / `--verbose` | count | `0` | Jan 9 | Wired (global verbosity) |
| `--workers` | int | `os.cpu_count()` | Feb 9 only | Wired (worker count) |
| `--memlog` | flag | off | Feb 9 only | Removed |
| `--mem-guard` / `--no-mem-guard` | flag | off | Feb 9 only | Removed |
| `--adaptive-workers` / `--no-adaptive-workers` | bool | `True` | Feb 9 only | Removed |
| `--pool` / `--no-pool` | bool | `True` | Feb 9 only | Removed |
| `--lazy-audio` / `--no-lazy-audio` | bool | `True` | Feb 9 only | Removed |
| `--adaptive-safety-factor` | float | `1.35` | Feb 9 only | Removed |
| `--adaptive-warmup` | int | `2` | Feb 9 only | Removed |
| `--force-cpu` | bool | `False` | Feb 9 only | Wired (no `--no-force-cpu` companion in BA3) |
| `--shared-models` / `--no-shared-models` | bool | `False` | Feb 9 only | Removed |

All commands except `avqi`, `setup`, and `version` use the `common_options`
decorator which adds positional `IN_DIR` and `OUT_DIR` arguments (both
`click.Path(exists=True, file_okay=False)`).

---

## Processing Commands

### `align`

Forced alignment: adds word-level timing bullets to existing CHAT transcripts.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--whisper` / `--rev` | exclusive pair | `--rev` | UTR engine selection | Hidden compat alias → `--utr-engine` |
| `--wav2vec` / `--whisper_fa` | exclusive pair | `--wav2vec` | FA engine selection | Hidden compat alias → `--fa-engine` |
| `--pauses` | flag | off | Add pauses between words | Wired |
| `--wor` / `--nowor` | bool | `True` | Write %wor tier | Wired (`default_value_t = true`) |
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations | Wired | **(Feb 9 only)** |

**Pipeline task:** `"fa"` (forced alignment).

### `transcribe`

Create transcripts from audio files via ASR.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--whisper_oai` / `--rev` | exclusive pair | `--rev` | ASR engine (OAI variant) | Hidden compat alias → `--engine openai` |
| `--whisper` / `--rev` | exclusive pair | `--rev` | ASR engine (HF variant) | Hidden compat alias → `--engine whisper` |
| `--whisperx` / `--rev` | exclusive pair | `--rev` | ASR engine (WhisperX variant) | **Removed in BA3.** WhisperX engine no longer ships; the BA2 flag is rejected. Use `--engine whisper` (HF) or `--engine openai` instead. |
| `--diarize` / `--nodiarize` | bool | `False` | Speaker diarization | Hidden compat alias → `--diarization` |
| `--wor` / `--nowor` | bool | `False` | Write %wor tier | Wired |
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |
| `--lang` | str | `"eng"` | Language code | Wired |
| `-n` / `--num_speakers` | int | `2` | Expected speaker count | Wired |

**Pipeline task:** `"asr"` (without diarization) or `transcribe_s` dispatch →
`"asr,speaker"` (with `--diarize`).

**Jan 9 behavior note:** the preserved BA2 CLI help text said `--diarize` was
"ignored with Rev.AI", but the implementation did not do that. CLI dispatch
still routed `--diarize` to `transcribe_s`, and `transcribe_s` still ran the
post-ASR speaker pipeline after Rev transcription.

**Note on `--wor`:** BA2 default was `False` (no `%wor`). Current BA3 preserves
that policy and wires `--wor` / `--nowor` through the Rust transcribe path.
Earlier migration-stage notes flagged this as a regression, but current code and
tests now cover the `%wor` toggle explicitly.

### `morphotag`

Morphosyntactic analysis (POS, lemma, dependency parse).

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--retokenize` / `--keeptokens` | bool | `False` | Retokenize main line for UD | Wired |
| `--skipmultilang` / `--multilang` | bool | `False` | Skip multilingual files | Wired |
| `--lexicon` | path | `None` | Manual lexicon override | Wired |
| `--override-media-cache` / `--use-cache` | bool | `False` | Bypass analysis cache **(Feb 9 only)** | Wired |
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |
| `--no-l2-morphotag` | flag | off | Opt out of BA3's default-on per-word `@s` secondary dispatch and keep legacy `L2\|xxx` placeholders | BA3-only |

**Pipeline task:** `"morphosyntax"`.

**Migration note:** `--skipmultilang` and `--no-l2-morphotag` are not
equivalent. `--skipmultilang` is the utterance-level `[- lang]` skip control;
`--no-l2-morphotag` is the BA3-only opt-out for per-word `@s` routing. BA3
also validates whole-utterance same-language all-`@s` patterns as E255 and
warns on explicit `@s:LANG` missing from `@Languages` as E254; `chatter debug
fix-s` repairs both transcript-side issues.

### `translate`

Translation to English.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |

**Pipeline task:** `"translate"`.

### `coref` (hidden)

Coreference resolution. Hidden from `--help` in BA2.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |

**Pipeline task:** `"coref"`.

### `utseg`

Utterance segmentation.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--lang` | str | `"eng"` | Language code | Wired |
| `-n` / `--num_speakers` | int | `2` | Expected speaker count | Wired |
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |

**Pipeline task:** `"utseg"`.

### `benchmark`

ASR word error rate benchmarking against gold transcripts.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--whisper` / `--rev` | exclusive pair | `--rev` | ASR engine (HF variant) | Hidden compat alias → `--asr-engine` |
| `--whisper_oai` / `--rev` | exclusive pair | `--rev` | ASR engine (OAI variant) | Hidden compat alias → `--asr-engine` |
| `--lang` | str | `"eng"` | Language code | Wired |
| `-n` / `--num_speakers` | int | `2` | Expected speaker count | Wired |
| `--wor` / `--nowor` | bool | `False` | Write %wor tier | Wired |
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |

**Pipeline task:** `"asr"` (transcribe) + `"morphosyntax"` (compare).

### `compare` **(Feb 9 only)**

Transcript comparison against gold-standard references.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--lang` | str | `"eng"` | Language code | Wired |
| `--merge-abbrev` / `--no-merge-abbrev` | bool | `False` | Merge abbreviations **(Feb 9 only)** | Wired |

**Pipeline task:** `"morphosyntax"` (compare uses morphosyntax to tag both transcripts before WER computation).

**Note on `--lang`:** BA2 passed `--lang` through the compare pipeline for
morphosyntax. Current BA3 also exposes `--lang` on `CompareArgs` and carries it
through compare dispatch. Earlier migration-stage notes flagged this as missing,
but the current CLI surface and tests cover it.

### `opensmile`

OpenSMILE acoustic feature extraction.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--feature-set` | choice | `"eGeMAPSv02"` | eGeMAPSv02 / eGeMAPSv01b / GeMAPSv01b / ComParE_2016 | Wired |
| `--lang` | str | `"eng"` | Language code | Wired |

**Note:** Uses its own `input_dir`/`output_dir` positional arguments instead of
`common_options`.

**Pipeline task:** `"opensmile"`.

### `avqi`

Acoustic Voice Quality Index from paired .cs/.sv audio files.

| Flag | Type | Default | Help | BA3 Status |
|------|------|---------|------|------------|
| `--lang` | str | `"eng"` | Language code | Wired |

**Note:** Uses its own `input_dir`/`output_dir` positional arguments instead of
`common_options`.

**Pipeline task:** `"avqi"`.

---

## Admin Commands

### `setup`

Interactive configuration wizard. Creates/updates `~/.batchalign.ini` with
default ASR engine and Rev.AI API key.

No command-specific flags in BA2. BA3 adds `--engine`, `--rev-key`, and
`--non-interactive` for scripted setup.

### `version`

Prints version and credits via `pyfiglet`.

No flags. BA3 equivalent: `batchalign3 version`.

---

## Utility Commands

### `cache` **(Feb 9 only)**

Cache management. Not present in the Jan 9 baseline. Registered as an external
Click subcommand from the BA2 `cache` CLI module in the Feb 9 master.

BA2 subcommands: `stats`, `clear`, `warm`. BA3 supports `stats` and `clear`
(with `--all` and `--yes` options).

### `bench` **(Feb 9 only)**

Repeated benchmark execution for performance measurement. Not present in the
Jan 9 baseline. Registered as an external Click subcommand from the BA2 `bench`
CLI module in the Feb 9 master.

BA3 equivalent: `batchalign3 bench <command> <in_dir> <out_dir> --runs N`.

### `models`

Model training utilities. Registered via `add_command` from
`batchalign.models.training.run`.

BA2 subcommand: `train`. BA3 adds `prep` (Rust-native training text extraction)
alongside `train` (Python runtime).

---

## batchalignHK Plugin (Archived)

The HK plugin was a separate PyPI package (`batchalign-hk-plugin`) that
registered additional ASR/FA engines via Python entry points. It was folded
into batchalign3 as built-in engines in March 2026; there is no separate HK
install tier now.

### Plugin Discovery

BA2 used `importlib.metadata.entry_points(group="batchalign.inference")` to
discover plugin-provided `InferenceProvider` implementations at startup. Each
provider registered `PluginDescriptor` objects declaring engine name, task type,
and factory function.

### Engines

| Engine | Task | Module | Credentials |
|--------|------|--------|-------------|
| `tencent` | ASR | `batchalign_hk.tencent_asr` | Tencent Cloud API key |
| `aliyun` | ASR | `batchalign_hk.aliyun_asr` | Aliyun NLS API key |
| `funaudio` | ASR | `batchalign_hk.funaudio_asr` | None (local model) |
| `wav2vec_canto` | FA | `batchalign_hk.cantonese_fa` | None (local model) |

### Selection

Engines were selected via `--engine-overrides '{"asr": "tencent"}'` on the
CLI. The JSON payload was parsed into a `BTreeMap<String, String>` and
forwarded to worker dispatch, which matched the engine name against plugin
registrations.

### BA3 Status

All four engines are now built-in modules under `python/batchalign/inference/languages/cantonese/`.
Engine dispatch uses `AsrEngine`/`FaEngine` enums in `worker/_types.py`.
The plugin discovery mechanism (`PluginDescriptor`, `InferenceProvider`,
entry points) has been completely removed. See
[Plugin Removal Notes](../developer/plugins.md) for the full migration record.

---

## Pipeline Task Mapping

| Command | Pipeline Task String | Notes |
|---------|---------------------|-------|
| `align` | `"fa"` | Forced alignment |
| `transcribe` | `"asr"` | Without diarization |
| `transcribe` (diarized) | `"asr"` + speaker | With `--diarize` |
| `morphotag` | `"morphosyntax"` | POS + lemma + depparse |
| `translate` | `"translate"` | Google Translate / Seamless M4T |
| `coref` | `"coref"` | English only, document-level |
| `utseg` | `"utseg"` | Constituency parse → boundaries |
| `benchmark` | `"asr"` + `"morphosyntax"` | Transcribe then compare |
| `compare` | `"morphosyntax"` | Tags both sides before WER |
| `opensmile` | `"opensmile"` | Feature extraction |
| `avqi` | `"avqi"` | Voice quality index |

---

## Regression Summary

No current CLI-surface regressions are recorded in this baseline table for
`transcribe`, `benchmark`, or `compare`.

Historical note: earlier BA3 migration notes temporarily flagged
`transcribe --wor`, `benchmark --wor`, and `compare --lang` as regressions
during the rewrite. Current code and tests now wire those surfaces.
