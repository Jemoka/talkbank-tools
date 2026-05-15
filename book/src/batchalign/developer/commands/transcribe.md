# transcribe — Developer Reference

**Status:** Current
**Last updated:** 2026-05-05 08:21 EDT

Implementation guide for the `transcribe` command. For user-facing
documentation, see [User Guide: transcribe](../../user-guide/commands/transcribe.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `TranscribeArgs` | ASR engine, diarization, lang, num-speakers |
| Options builder | `crates/batchalign/src/cli/args/options.rs:195–243` (inline dispatch) | Maps `TranscribeArgs` → `CommandOptions::Transcribe(TranscribeOptions)` |
| Command definition | `crates/batchalign/src/commands/transcribe.rs` | `CommandDefinition` impl |
| Pipeline orchestration | `crates/batchalign/src/pipeline/transcribe.rs` — `run_transcribe_pipeline()` | 7-stage sequencer: ASR → post-process → (opt) diarization → build CHAT → (opt) utseg → (opt) morphotag → serialize |
| Per-file dispatch | `crates/batchalign/src/runner/dispatch/transcribe_pipeline.rs` | Concurrent file orchestration bounded by semaphore |
| ASR post-processing | `crates/talkbank-transform/src/asr_postprocess/mod.rs` | 8 stages: compound merge, MWT split, number expand, Cantonese norm, long-turn split, retokenization, disfluency, retrace detection |
| Pre-CHAT utterance segmentation | `crates/batchalign/src/pipeline/transcribe.rs:421–457` — `process_asr_with_prechat_segmentation()` | Runs for eng/cmn/zho/yue: BERT utseg applied to prepared chunks BEFORE build_chat |
| CHAT assembly | `crates/talkbank-transform/src/build_chat/mod.rs:41` — `build_chat()` | Assembles `ChatFile` AST from `TranscriptDescription` (typed bridge) |
| Speaker reassignment | `crates/batchalign/src/chat_ops/speaker.rs:32` — `reassign_speakers()` | Rewrites speaker codes + headers from diarization segments (runs post-build_chat) |
| ASR worker IPC | `python/batchalign/inference/asr.py` | Whisper/Rev.AI ASR, returns raw tokens |
| Speaker worker IPC | `python/batchalign/inference/speaker.py` — `batch_infer_speaker()` | Pyannote/NeMo diarization, returns speaker segments |

---

## ASR post-processing chain

All ASR post-processing runs in Rust (`crates/talkbank-transform/src/asr_postprocess/`). The pipeline is deterministic and language-aware.

### 8-stage pipeline

1. **Compound merging** — rejoin compound words split by ASR
   - Language-specific: English phrasal verbs, CJK terms, etc.
   - Implemented: `compounds::merge_compounds()`

2. **Multi-word token splitting** — split tokens containing spaces, interpolate timestamps
   - Normalizes ASR outputs that glue multiple words together
   - Distributes timing proportionally by text length

3. **Number expansion** — convert digit strings to word form
   - Cardinals: 47 languages via `NUM2LANG` static table (data/num2lang.json)
   - CJK: specialized `num2chinese` path
   - Ordinals/decades: English-specific `ordinal_year_eng` composer
   - Currency, percent, dash-ranges: dedicated Rust handlers
   - **Runtime:** Pure Rust table lookup (Python `num2words` involved at **build time only** for codegen; removed from runtime 2026-04-26)

4. **Cantonese normalization** (yue only) — simplified→HK traditional + domain replacements
   - Uses `ferrous-opencc` crate + replacement table
   - Implemented: `cantonese::normalize()`

5. **Long-turn splitting** — chunk monologues >300 words
   - Prevents unbounded utterance lengths in downstream processing

6. **Retokenization** — punctuation-based utterance splitting
   - Splits by CHAT-legal sentence terminators (`.` `?` `!` `+...` etc.)
   - Handles long-pause splitting when ASR omits punctuation

7. **Disfluency replacement** — mark filled pauses and orthographic variants
   - Filled pauses: `"um"` → `"&-um"`, `"uh"` → `"&-uh"` (per-language wordlists)
   - Replacements: `"'cause"` → `"(be)cause"`, `"gonna"` → `"going to"` (CJK-aware)
   - Implemented: `cleanup::mark_disfluencies()`

8. **N-gram retrace detection** — detect repeated n-grams, wrap in `<...> [/]` annotation
   - Identifies speaker self-corrections (rephrasings)
   - Implemented: `cleanup::detect_retraces()`

---

## Pre-CHAT utterance segmentation (lang-specific)

For **eng, cmn, zho, yue**, a BERT-based utterance segmentation model runs **after ASR post-processing** but **before CHAT assembly**:

- Implemented in `crates/batchalign/src/pipeline/transcribe.rs:421–457` — `process_asr_with_prechat_segmentation()`
- Called only when `uses_prechat_utterance_model(resolved_lang)` is true (lines 387–389)
- Workflow:
  1. Prepare ASR chunks (stages 1–8 above)
  2. Call `infer_utseg_assignments()` to get per-chunk segment boundaries from worker
  3. Apply `split_prepared_chunk_by_assignments()` to split chunks at boundaries
  4. Convert to final utterances + finalize
- **Purpose:** Improve sentence boundary detection for languages with ambiguous punctuation
- For all other languages: skip pre-CHAT segmentation; use punctuation-based retokenization only

---

## Worker IPC: ASR task (V2 protocol)

```text
execute_v2 request:
{
  "task": "asr",
  "prepared_audio": { path, start_ms, end_ms, sample_rate },
  "engine": "rev" | "whisper" | "whisperx" | "whisper_oai" | "tencent" | ...,
  "language": "eng",
  "num_speakers": 2
}

execute_v2 response:
{
  "tokens": [
    { "word": "hello", "start_s": 0.12, "end_s": 0.45,
      "speaker": "SPEAKER_00", "confidence": 0.98 },
    ...
  ]
}
```

The speaker field is optional — Rev.AI always provides it; Whisper omits it.

## Worker IPC: speaker task (V2 protocol)

When `--diarization enabled` is set, a second worker call runs after ASR:

```text
execute_v2 request:
{
  "task": "speaker",
  "prepared_audio": { path, ... },
  "num_speakers": 2
}

execute_v2 response:
{
  "segments": [
    { "start_s": 0.0, "end_s": 2.3, "speaker": "SPEAKER_00" },
    ...
  ]
}
```

`reassign_speakers()` in `crates/batchalign/src/chat_ops/speaker.rs` then relabels utterances using
these segments as the authoritative source.

---

## Language resolution flow

When the user specifies `--lang auto`, language detection happens in two phases:

### Phase 1: ASR-level detection
- ASR worker returns detected `lang` field in response (e.g., "spa", "fra", "eng")
- This becomes the **resolved language** for CHAT headers and NLP stages (utseg, morphosyntax)
- Implemented: `resolved_asr_language()` in `crates/batchalign/src/pipeline/transcribe.rs:362–385`

### Phase 2: Per-utterance code-switching detection (if lang=auto)
- During `build_chat()`, each utterance text is analyzed with `lang_detect::detect_utterance_language()`
- Detected language stored in `Utterance.lang` field
- Used to emit `[- lang]` code-switching precodes in CHAT tier (if different from resolved language)
- Implemented: `build_chat()` stage lines 629–657

For fixed languages (not auto):
- No per-utterance detection; entire file uses specified language
- No code-switching precodes emitted

---

## Rev.AI `skip_postprocessing` gate

For `lang == eng || lang == fra`, Rev.AI is called with
`skip_postprocessing=true`. This suppresses Rev.AI's built-in punctuation
so that BA3's BERT utseg model handles sentence boundary detection. For all
other languages, Rev.AI post-processing is applied. Gate implemented in
`python/batchalign/inference/asr.py` — `_revai_request()`.

---

## `transcribe_s` vs `transcribe`

`transcribe_s` is not a separate CLI command. It is an internal command
variant triggered by `--diarization enabled`. Both share the same
`transcribe_pipeline.rs` orchestrator; the only difference is whether the
dedicated speaker stage runs.

---

## Testing

```bash
# Fast unit tests (no ML models)
bazel test //...

# Transcribe golden tests (real ASR models — only on net)
cargo nextest run --profile ml -E 'test(transcribe::)'

# Python ASR inference tests
uv run pytest batchalign/tests/test_asr.py -m golden
```

---

## Related developer documentation

- [Command Flowcharts: transcribe](../../architecture/command-flowcharts.md#transcribe) — detailed runtime flowchart
- [ASR Token Pipeline](../../architecture/asr-token-pipeline.md) — post-processing details
- [Cantonese and CJK — Architecture](../../../architecture/language-and-multilingual/cantonese-and-cjk.md) — Tencent, Aliyun, FunASR engine dispatch
- [Number Expansion](../../reference/number-expansion.md) — per-language Rust expansion
