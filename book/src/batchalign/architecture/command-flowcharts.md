# Command Flowcharts

**Status:** Current
**Last updated:** 2026-06-01 00:51 PDT

Option-driven flowcharts for every batchalign processing command. Each
diagram shows how CLI flags route through different code paths at runtime.
For the higher-level dispatch sequence diagrams, see
[Command Lifecycles](command-lifecycles.md).

---

## align

The most complex command. CLI flags control FA engine selection, timing
mode, UTR pre-pass behavior, and incremental processing.

```mermaid
flowchart TD
    start([align invoked]) --> read[Read CHAT file]
    read --> resolve_audio[Resolve audio file]
    resolve_audio --> ensure_wav[ensure_wav — convert mp4→wav if needed]
    ensure_wav --> parse[parse_lenient → ChatFile]
    parse --> reuse_check{Complete reusable\n%wor timing?}
    reuse_check -->|Yes| reuse[Refresh main-tier bullets\nfrom %wor + optionally\nregenerate %wor]
    reuse_check -->|No| count[count_utterance_timing → timed, untimed]
    reuse --> done([Output .cha file])

    count --> utr_check{untimed > 0?}
    utr_check -->|No| skip_utr[Skip UTR — all timed]
    utr_check -->|Yes| utr_engine_check{--utr engine\nconfigured?}

    utr_engine_check -->|Yes: --utr| run_utr_pass["run_utr_pass()"]
    utr_engine_check -->|No: --no-utr| warn_interp[Log warning\nFall back to interpolation]

    run_utr_pass --> utr_done[Re-serialize CHAT\nwith recovered timing]
    utr_done --> group

    warn_interp --> group
    skip_utr --> group

    group[group_utterances → time windows]

    group --> before_check{--before path\nprovided?}
    before_check -->|Yes| incremental[process_fa_incremental\nDiff old vs new, copy stable %wor,\nreuse preserved groups]
    before_check -->|No| full[process_fa\nProcess all groups]

    incremental --> engine_select
    full --> engine_select

    engine_select{--fa-engine?}
    engine_select -->|whisper| whisper_fa[WhisperFa engine\nmax_group_ms=20000]
    engine_select -->|wav2vec| wav2vec_fa[Wave2Vec engine\nmax_group_ms=15000]

    whisper_fa --> pause_check{--pauses?}
    pause_check -->|Yes| with_pauses[FaTimingMode::WithPauses]
    pause_check -->|No| continuous_w[FaTimingMode::Continuous]

    wav2vec_fa --> continuous_wv[FaTimingMode::Continuous]

    with_pauses --> cache_check
    continuous_w --> cache_check
    continuous_wv --> cache_check

    cache_check[Cache lookup — BLAKE3 keys]
    cache_check --> worker_infer[execute_v2(task="fa") misses → Python FA worker\nprepared audio + prepared text]
    worker_infer --> dp_align_fa[DP-align model output → transcript words]
    dp_align_fa --> inject_fa[Inject word-level timings into AST]

    inject_fa --> retry_check{FA\nsucceeded?}
    retry_check -->|Yes| wor_check
    retry_check -->|No + retryable| fallback_check{Untimed utts\nnot recovered?}
    fallback_check -->|Yes + not tried| fallback_utr["Fallback: run_utr_pass()\n(at most once)"]
    fallback_utr --> retry_loop[Retry FA with\nrecovered timing]
    retry_loop --> cache_check
    fallback_check -->|No or already tried| backoff[Backoff + retry]
    backoff --> cache_check

    wor_check{--wor / --nowor?}
    wor_check -->|--wor| gen_wor[Generate %wor tier]
    wor_check -->|--nowor| skip_wor[Omit %wor tier]

    gen_wor --> merge_check
    skip_wor --> merge_check

    merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations transform]
    merge_check -->|No| validate

    merge --> validate[Post-validate → serialize CHAT output]
    validate --> done([Output .cha file])
```

### UTR Detail: Strategy Selection (Auto disabled)

When `--utr-strategy auto` (the default), the strategy is currently always
`GlobalUtr` regardless of file content or language. Two-pass overlap-aware
recovery is reachable only via the explicit `--utr-strategy two-pass`
override.

```mermaid
flowchart TD
    auto(["--utr-strategy auto\n(default)"]) --> always_global["GlobalUtr\n(monotonic single-pass)"]
    explicit_global(["--utr-strategy global"]) --> force_global["GlobalUtr\n(explicit override)"]
    explicit_two(["--utr-strategy two-pass"]) --> force_two["TwoPassOverlapUtr\n(explicit override)"]
```

**Why Auto is currently disabled (2026-03-30):** the previous content/
language-aware gate auto-picked `TwoPassOverlapUtr` for English files
containing `+<` or CA overlap markers. It was disabled after an operator
reported alignment regressions on real files; investigation found that
`enforce_monotonicity()` only checks start times, not end times, so
overlapping utterance bullets go uncorrected. The two-pass tuning was also
based on four corpora and never broadly validated. The previously-measured
gains under that mechanism (English: +4.3pp SBCSAE, +3.8pp Jefferson;
non-English on Hakka/Welsh/German/Serbian: GlobalUtr matched or beat
TwoPassOverlapUtr) are retained here as historical context for the
benchmark numbers that motivated the original gate, not as a description
of current behavior.

**Implementation:** `resolve_strategy()` in
`crates/batchalign/src/runner/dispatch/utr.rs` (the inline comment in the
`Auto` arm carries the disable rationale). The language-agnostic
overlap-detection helper `select_strategy()` in
`crates/batchalign/src/chat_ops/fa/utr.rs` (library) remains, but is no
longer called from the `Auto` path.

### UTR Detail: `run_utr_pass()` internals

The UTR pre-pass and fallback share the same `run_utr_pass()` helper,
which chooses between full-file and partial-window ASR:

```mermaid
flowchart TD
    entry(["run_utr_pass()"]) --> parse[Parse CHAT\ncount timed vs untimed]
    parse --> zero{untimed == 0?}
    zero -->|Yes| noop([Return — nothing to do])
    zero -->|No| ratio{untimed < 50%\nAND audio > 60s?}

    ratio -->|Yes| partial_mode

    subgraph partial_mode [Partial-Window ASR]
        direction TB
        pw_find[find_untimed_windows\nPadding: 500ms, merge overlaps]
        pw_find --> pw_loop["For each window (start, end):"]
        pw_loop --> pw_seg_cache{Segment\ncache hit?}
        pw_seg_cache -->|Hit| pw_use[Use cached segment ASR]
        pw_seg_cache -->|Miss| pw_extract["extract_audio_segment()\nffmpeg -ss/-to → cached WAV"]
        pw_extract --> pw_infer[infer_asr on segment]
        pw_infer --> pw_store[Cache segment result]
        pw_store --> pw_use
        pw_use --> pw_offset[Offset token times\nby window start_ms]
        pw_offset --> pw_loop
    end

    ratio -->|No| full_mode

    subgraph full_mode [Full-File ASR]
        direction TB
        ff_cache{Full-file\ncache hit?}
        ff_cache -->|Hit| ff_use[Use cached ASR]
        ff_cache -->|Miss| ff_infer[infer_asr on full audio]
        ff_infer --> ff_store[Cache full result]
        ff_store --> ff_use
    end

    partial_mode --> inject
    full_mode --> inject

    inject["inject_utr_timing()\nExact-subsequence fast path,\nelse global DP"]
    inject --> result([Return updated CHAT + UtrResult])
```

### UTR Detail: Zero-Duration Bullet Prevention

Zero-duration utterance bullets (`•T_T•`, where start_ms == end_ms) fail E362
validation and **perpetuate across every subsequent `align` re-run**: the FA
postprocess clamps all word timings to the utterance range `[T, T]`, drops
every word bullet (nothing survives the `start >= end` check), and
`update_utterance_bullet` then has nothing to work from — so the zero-duration
bullet is preserved unchanged.

The root cause is **Whisper's 20ms DTW grid**, which can return the same
timestamp for multiple adjacent short words ("mhm", "yeah"). This affects
short backchannels in dense-dialogue corpora (first seen in OCSC).

BA3 applies a **three-layer defence** in `crates/batchalign/src/chat_ops/fa/utr.rs`
and `crates/batchalign/src/chat_ops/fa/orchestrate.rs`:

```mermaid
flowchart TD
    asr["ASR token stream\n(Whisper DTW output)"] --> filter

    subgraph L1 ["Layer 1 — Token filter (dispatch/utr.rs)"]
        filter{"token.end_ms\n<= token.start_ms?"}
        filter -->|Yes| drop1[Drop token before UTR\nsees it at all]
        filter -->|No| utr_in[Pass to UTR]
    end

    utr_in --> dp["Global DP alignment\nCHAT words ↔ ASR tokens"]
    dp --> assign["Assign per-utterance\ntoken range (min_asr, max_asr)"]

    subgraph L2 ["Layer 2 — UTR span guard (utr.rs run_global_utr)"]
        assign --> zdcheck{"asr[min].start_ms\n>= asr[max].end_ms?"}
        zdcheck -->|Yes| drop2[Leave utterance untimed\ncount as unmatched]
        zdcheck -->|No| mono_check
    end

    subgraph L3 ["Layer 3 — UTR monotonicity pass (utr.rs run_global_utr)"]
        mono_check{"utt.start_ms\n< prev_non_overlap.end_ms?"}
        mono_check -->|Yes — DTW collision| advance["Advance start_ms = prev.end_ms\nExtend end_ms if needed"]
        mono_check -->|No| assign_bullet[Assign utterance bullet]
        advance --> assign_bullet
    end

    assign_bullet --> fa_in["FA postprocess\n(orchestrate.rs)"]

    subgraph L4 ["Safety net — monotonicity enforcement (orchestrate.rs)"]
        fa_in --> mono_enforce{"prev.end_ms\n> next.start_ms?"}
        mono_enforce -->|end_clamp safe| clamp_end[Clamp prev.end_ms\nto next.start_ms]
        mono_enforce -->|would produce\nzero-duration| strip[Strip bullet entirely\nbetter untimed than •T_T•]
    end
```

**Why three layers instead of one?**

Each layer catches a different failure mode:

| Layer | Where | What it catches |
|-------|--------|----------------|
| 1 | `dispatch/utr.rs` `asr_response_to_utr_tokens` | Whisper returning `start==end` for a single-frame token |
| 2 | `utr.rs` `run_global_utr` bullet assignment | DP aligning an utterance to an ASR token range whose span is zero or negative |
| 3 | `utr.rs` `run_global_utr` monotonicity post-pass | Two adjacent non-overlap utterances assigned to tokens with the same `start_ms` (DTW collision at a shared boundary) |
| Safety net | `orchestrate.rs` `enforce_monotonicity` | Residual overlaps from any source; when clamping would produce zero-duration, strip entirely |

Layer 3 is the **root-cause fix**. Layers 1 and 2 handle degenerate single-token
cases. The safety net handles anything that slips through (e.g. cross-speaker
overlap that is not marked with `+<`).

**Why not just rely on the safety net?**  Because stripping a bullet destroys
timing — the utterance goes back to untimed and must be recovered by FA.  The
UTR-level fixes preserve timing: advancing `start_ms` to `prev.end_ms` keeps
both utterances timed and valid.

### BulletSource Provenance: The Self-Healing Design

The three layers above prevent UTR from producing bad bullets. A complementary
mechanism ensures that even if UTR set a slightly imprecise window, **FA word
timings are authoritative after alignment**.

Every `Bullet` carries a non-serialized `source: BulletSource` field (in
`talkbank-model/src/model/content/bullet.rs`):

| `BulletSource` | Who sets it | `update_utterance_bullet` behavior |
|---|---|---|
| `Utr` | UTR pre-pass via `Bullet::utr_hint()` | **Overwrite** with FA word span |
| `Authoritative` | Parser (hand-linked), `Bullet::new()`, or FA-derived | **Union** (never shrink) |

`BulletSource` is `#[serde(skip)]` — it never appears in CHAT output and
doesn't change the file format.

```mermaid
flowchart TD
    utr_bullet["UTR sets\nBullet::utr_hint(800, 3000)\nBulletSource::Utr"]
    hand_bullet["Parser reads hand-linked bullet\nBullet::new(37397, 42983)\nBulletSource::Authoritative"]

    utr_bullet --> fa_inject["FA injects per-word timings\n1000_1500, 1500_2000"]
    hand_bullet --> fa_inject

    fa_inject --> update["update_utterance_bullet()"]

    update --> check{"source?"}
    check -->|"Utr"| overwrite["Overwrite: bullet = 1000_2000\nFA span is authoritative"]
    check -->|"Authoritative"| union_op["Union: bullet = 37397_42983\npreserves filler/gesture coverage"]
    check -->|"None"| set["Set: bullet = word span"]

    overwrite --> auth["Mark result as Authoritative"]
    union_op --> auth
    set --> auth
```

**Why union for authoritative bullets?** Hand-linked utterances may start
before the first FA-alignable word (e.g., `&-uh` filler that FA returns `None`
for) or end after the last word (e.g., a trailing `&=laughs` gesture). Without
union, re-running FA on these utterances would silently shrink their timing,
losing the hand-annotated context coverage. This was a real bug
encountered in the ACWT corpus. The `BulletSource` design preserves
the correct
behavior for authoritative bullets while enabling the self-healing property
for UTR hints.

**Contrast with batchalign2 (jan9 baseline, commit `84ad500b`):**

BA2 uses the same conceptual approach — DP alignment of ASR tokens against the
reference transcript, utterance-level timing derived from word-level timing —
but its implementation differs in two important ways:

1. **Utterance timing is derived dynamically, not stored.** `Utterance.alignment`
   in `document.py:182` is a computed property: it scans forward/backward
   through `word.time` to find the first and last timed word.  There is no
   explicit utterance bullet; the CHAT serializer writes it on demand.  This
   means a zero-duration utterance bullet can only arise if the first and last
   timed words in an utterance share a timestamp — which BA2 prevents at the
   **word level** inside `whisper_fa.py:183–224`: each word's `end_ms` is set to
   the start of the next word, and any word where `start >= end` is dropped
   (`word.time = None`).

2. **No cross-utterance monotonicity enforcement.** BA2 performs no check that
   `utt_n.alignment[0] >= utt_{n-1}.alignment[1]`.  Adjacent utterances can and
   do share start timestamps from DTW collisions; BA2 tolerates this because it
   never runs an `enforce_monotonicity` pass that would turn the shared start
   into a zero-duration span.

**Why BA3 needs explicit monotonicity enforcement:** BA3 separates UTR (bullet
assignment) from FA (word-level timing), whereas BA2 derives utterance timing
from word timing.  This means BA3 can produce utterance bullets that are valid
on their own but conflict with each other (same `start_ms`), and the
`enforce_monotonicity` end-clamp pass — which BA2 does not have — converts
those into zero-duration spans.  The Layer 3 fix eliminates the conflict at the
source before `enforce_monotonicity` ever sees it.

---

## transcribe

Creates CHAT from audio. The longest pipeline, with optional follow-up
commands chained automatically.

**When do you need `--diarize`?**
- **Rev.AI (the default engine):** Rev.AI returns multi-speaker labels
  natively as part of its ASR response. Those labels are **always** applied
  to the transcript — you get multi-speaker output without `--diarize`.
- **Rev.AI with explicit `--diarize`:** BA3 now matches the audited Jan 9 BA2
  implementation. If you explicitly request diarization, BA3 still runs the
  separate Pyannote/NeMo post-ASR speaker stage on top of Rev output.
- **Whisper-based engines** (`whisper`, `chatwhisper`, `openai`): these
  engines do not return speaker labels. Passing `--diarize` (or
  `--diarization enabled`) runs a dedicated Pyannote speaker model as an
  additional stage.
- Default: `--diarization auto` = disabled. Identical to batchalign2's
  `--diarize/--nodiarize default=False`. The old BA2 help text claiming Rev
  ignored `--diarize` was stale; the pipeline wiring did not ignore it.

**Rev.AI `skip_postprocessing`:** For English (`en`) and French (`fr`),
Rev.AI is called with `skip_postprocessing=true`, matching BA2. This lets
BA3's own BERT utterance segmentation model handle sentence boundaries from
raw ASR output, rather than relying on Rev.AI's built-in punctuation which
produces giant monologue blobs. For all other languages, Rev.AI applies its
own post-processing.

```mermaid
flowchart TD
    start([transcribe invoked]) --> resolve[Resolve audio file]
    resolve --> ensure_wav[ensure_wav — convert if needed]

    ensure_wav --> diarize_check{--diarization?}
    diarize_check -->|"enabled"| transcribe_s["Command: transcribe_s\nASR + dedicated speaker relabeling\nRev or Whisper"]
    diarize_check -->|"auto/disabled\n(default)"| transcribe_m["Command: transcribe\nDefault path\nRev labels used directly when present"]

    transcribe_s --> engine_check
    transcribe_m --> engine_check

    engine_check{--engine?}
    engine_check -->|whisper| whisper["HF Whisper local ASR"]
    engine_check -->|chatwhisper| chatwhisper["CHATWhisper\n(TalkBank fine-tune)"]
    engine_check -->|rev| rev_preflight["Rev.AI preflight\nPre-submit audio in parallel\nskip_postprocessing=true for en/fr"]
    engine_check -->|openai| whisper_oai["OpenAI Whisper API (turbo)"]
    engine_check -->|funaudio| funaudio["FunAudio SenseVoice"]
    engine_check -->|tencent| tencent["Tencent ASR"]
    engine_check -->|qwen3| qwen3["Qwen3-ASR"]

    rev_preflight --> rev_poll[Poll Rev.AI for results]
    rev_poll --> asr_tokens

    whisper --> asr_tokens
    chatwhisper --> asr_tokens
    whisper_oai --> asr_tokens
    funaudio --> asr_tokens
    tencent --> asr_tokens
    qwen3 --> asr_tokens

    asr_tokens["Raw ASR tokens\nword + start_s + end_s + optional speaker + confidence"]
    asr_tokens --> convert["convert_asr_response()\nALWAYS groups tokens by speaker label\nNo use_speaker_labels parameter"]
    convert --> dedicated_check{"--diarization enabled?"}
    dedicated_check -->|No| postprocess
    dedicated_check -->|Yes| speaker_v2["execute_v2(task=speaker)\nprepared audio → raw diarization segments\nPost-ASR relabeling via Pyannote or NeMo"]
    speaker_v2 --> postprocess

    subgraph postprocess ["Rust post-processing: process_raw_asr()"]
        direction TB
        p1[1. Compound merging] --> p2[2. Timed word extraction\nseconds → milliseconds]
        p2 --> p3[3. Multi-word splitting\ntimestamp interpolation]
        p3 --> p4[4. Number expansion\ndigits → word form]
        p4 --> p4check{lang=yue?}
        p4check -->|Yes| p4b["4b. Cantonese normalization\nOpenCC + domain replacements"]
        p4check -->|No| p5
        p4b --> p5[5. Long-turn splitting\nchunk at >300 words]
        p5 --> p6[6. Retokenization\npunctuation-based utterance splitting]
    end

    postprocess --> build_chat["build_chat → ChatFile AST\nHeaders, participants, %wor tiers\nSpeaker codes from ASR labels: PAR, INV, CHI, ..."]
    build_chat --> speaker_apply{Dedicated speaker\nsegments present?}
    speaker_apply -->|Yes| reassign["reassign_speakers()\nRewrite utterance speakers +\n@Participants + @ID headers\nfrom raw diarization segments"]
    speaker_apply -->|No| utseg_check{"with_utseg?\ndefault: true"}
    reassign --> utseg_check

    utseg_check -->|Yes| run_utseg[process_utseg\nBERT-based re-segmentation]
    utseg_check -->|No| mor_check

    run_utseg --> mor_check{"with_morphosyntax?\ndefault: false"}
    mor_check -->|Yes| run_mor[process_morphosyntax\nPOS + lemma + depparse]
    mor_check -->|No| merge_check

    run_mor --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| output

    merge --> output[Serialize → .cha output]
    output --> done([Output .cha file])
```

---

## morphotag

Adds `%mor` and `%gra` tiers. Files are processed independently and
concurrently (bounded by `num_workers`).

```mermaid
flowchart TD
    start([morphotag invoked]) --> parse[Parse file → AST]
    parse --> ca_check{"@Options: CA\nin header?"}
    ca_check -->|Yes| ca_passthrough[Serialize parsed file as-is\nNo %mor/%gra added\nNo provenance injected]
    ca_passthrough --> done
    ca_check -->|No| clear[Clear existing %mor/%gra tiers]
    clear --> collect[collect_payloads\nPer-utterance word lists with language metadata]
    collect --> retok_check{--retokenize?}
    retok_check -->|Yes: --retokenize| stanza_retok[TokenizationMode::StanzaRetokenize\nStanza may split/merge words]
    retok_check -->|No: --keeptokens| preserve[TokenizationMode::Preserve\nKeep original tokenization]

    stanza_retok --> lang_check
    preserve --> lang_check

    lang_check{--skipmultilang?}
    lang_check -->|Yes| skip_non_primary[MultilingualPolicy::SkipNonPrimary\nSkip utterances in non-primary language]
    lang_check -->|No: --multilang| process_all[MultilingualPolicy::ProcessAll\nProcess all utterances regardless of language]

    skip_non_primary --> cache
    process_all --> cache

    cache[Cache lookup — BLAKE3 keys\nwords + lang + terminator + special forms + engine version]
    cache --> inject_hits[Inject cache hits immediately]
    inject_hits --> worker[execute_v2(task="morphosyntax") misses\nprepared_text batch → Stanza NLP pipeline]
    worker --> inject_results[inject_results → insert %mor/%gra tiers]

    inject_results --> before_check{--before path?}
    before_check -->|Yes| incremental[process_morphosyntax_incremental\nSkip NLP for unchanged utterances]
    before_check -->|No| full_inject[Process all utterances]

    incremental --> merge_check
    full_inject --> merge_check

    merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| validate

    merge --> validate[Alignment validation\n%mor word count must match main tier]
    validate --> done([Output .cha file])
```

**`@Options: CA` pass-through:** When the file's header declares
`@Options: CA`, the pipeline skips morphotagging entirely and serializes
the parsed file unchanged (mirroring `@Options: NoAlign` for `align`).
The decision is made once per file from the option header; no
per-utterance content scan is involved.

---

## utseg

Utterance segmentation. Pools all utterances across files into a single
GPU batch.

```mermaid
flowchart TD
    start([utseg invoked]) --> parse[Parse all files → ASTs]
    parse --> collect[collect_payloads\nExtract word sequences per utterance]
    collect --> cache[Cache lookup — BLAKE3 keys\nwords + lang]
    cache --> worker[execute_v2(task="utseg") misses\nprepared_text batch → raw parse trees]
    worker --> apply[Apply segmentation\nSplit/merge utterances at predicted boundaries]
    apply --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| serialize
    merge --> serialize[Serialize → .cha output]
    serialize --> done([Output .cha files])
```

---

## translate

Translates utterances and injects `%xtra` tiers.

```mermaid
flowchart TD
    start([translate invoked]) --> parse[Parse all files → ASTs]
    parse --> collect[collect_payloads\nExtract utterance text + source/target language]
    collect --> cache[Cache lookup — BLAKE3 keys\ntext + src_lang + tgt_lang]
    cache --> worker[execute_v2(task="translate") misses\nprepared_text batch → raw translations]
    worker --> inject[inject %xtra tiers with translated text]
    inject --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| serialize
    merge --> serialize[Serialize → .cha output]
    serialize --> done([Output .cha files])
```

---

## coref

Coreference resolution. Document-level, sparse output, English-only.

```mermaid
flowchart TD
    start([coref invoked]) --> parse[Parse all files → ASTs]
    parse --> collect[collect_payloads\nExtract sentences — full document context]
    collect --> worker[execute_v2(task="coref")\nprepared_text batch → structured chain refs]
    worker --> inject[inject %xcoref tiers — sparse\nOnly utterances with coreferent mentions]
    inject --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| serialize
    merge --> serialize[Serialize → .cha output]
    serialize --> done([Output .cha files])

    style collect fill:#ffd,stroke:#aa0
    note1[No caching — full-document context\nmakes per-utterance keys meaningless]
    collect --- note1
```

---

## compare

Reference-projection workflow. The released command now emits the projected
reference transcript, and the benchmark/internal main-shaped path is a separate
materializer rather than the command contract.

```mermaid
flowchart TD
    start([compare invoked]) --> discover[Discover primary .cha files\nskip *.gold.cha companions]
    discover --> pair[Pair FILE.cha with FILE.gold.cha]
    pair --> found{Gold companion found?}
    found -->|No| fail[Report file error]
    found -->|Yes| morph[process_morphosyntax\nmain transcript only]
    pair --> parse_gold[parse_lenient raw gold\n→ gold AST]
    morph --> parse_main[parse_lenient morphotagged main\n→ main AST]
    parse_main --> bundle[compare()\nconform + local window search + local DP\nComparisonBundle: main view, gold view,\nstructural word matches, metrics]
    parse_gold --> bundle
    bundle --> released[GoldProjectedCompareMaterializer\nproject_gold_structurally()]
    bundle --> internal_main[MainAnnotatedCompareMaterializer (internal/benchmark)\ninject %xsrep / %xsmor on main]
    released --> safe{Exact structural match?}
    safe -->|Yes| copy[Copy %mor / %gra / %wor]
    safe -->|No, full gold coverage| mor_only[Project %mor only]
    safe -->|No, partial or unsafe| keep[Keep gold dependent tiers unchanged]
    copy --> goldannot[Inject %xsrep / %xsmor on gold]
    mor_only --> goldannot
    keep --> goldannot
    goldannot --> merge_check
    internal_main --> internal_done([Internal main-annotated view])
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| metrics[Write .compare.csv]
    merge --> metrics
    metrics --> done([Output .cha + .compare.csv])
```

The public command now uses the projected-reference branch. The main-annotated
branch remains available only for internal consumers such as benchmark.

---

## opensmile

Acoustic feature extraction. Rust resolves media, prepares typed audio, and
sends a live V2 request to the Python worker.

```mermaid
flowchart TD
    start([opensmile invoked]) --> resolve[Resolve audio files]
    resolve --> prep[Rust audio prep\nprepare mono PCM artifact]
    prep --> feature_check{--feature-set?}
    feature_check -->|eGeMAPSv02| egemaps[eGeMAPSv02 features\n88 acoustic descriptors]
    feature_check -->|ComParE_2016| compare[ComParE_2016 features\n6,373 acoustic descriptors]
    feature_check -->|Custom| custom[Custom feature set name]

    egemaps --> worker
    compare --> worker
    custom --> worker

    worker[execute_v2(task=\"opensmile\") → Python worker\nExtracts acoustic features from prepared audio]
    worker --> output[Write CSV output\nContent-type: csv]
    output --> done([Output .csv files])
```

---

## avqi

Acoustic Voice Quality Index. Rust resolves paired audio, prepares typed PCM
artifacts, and sends a live V2 request. Requires paired continuous speech
(`.cs.wav`) and sustained vowel (`.sv.wav`) audio.

```mermaid
flowchart TD
    start([avqi invoked]) --> resolve[Resolve paired audio files\n.cs.wav + .sv.wav per speaker]
    resolve --> prep[Rust audio prep\nprepare CS + SV PCM artifacts]
    prep --> worker[execute_v2(task=\"avqi\") → Python worker\nparselmouth + torchaudio analysis]
    worker --> output[Write AVQI results\nHarmonics-to-noise ratio, jitter, shimmer, etc.]
    output --> done([Output results])
```

---

## benchmark

Composite workflow that runs transcribe, then compare, and materializes both
the hypothesis CHAT and the CSV metrics.

```mermaid
flowchart TD
    start([benchmark invoked]) --> resolve[Resolve audio file + companion gold .cha]
    resolve --> transcribe[Rust transcribe workflow\nProduce hypothesis CHAT]
    transcribe --> compare[Rust compare workflow\nDP alignment + WER metrics]
    compare --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[Merge abbreviations in hypothesis CHAT output]
    merge_check -->|No| output
    merge --> output[Write hypothesis .cha + .compare.csv]
    output --> done([Output results])
```

---

There is no standalone CLI `speaker` command in batchalign3, matching
batchalign2. User-facing diarization remains part of `transcribe_s`; the
low-level `speaker` worker task is documented in the worker-protocol V2 and
interface chapters instead of here.

---

## Cross-Cutting: Cache Behavior

All processing commands (except coref) follow this cache interaction
pattern. The cache policy is controlled by `--override-media-cache`.

```mermaid
flowchart TD
    start([Cache check]) --> policy{--override-media-cache?}
    policy -->|No| lookup[BLAKE3 hash → cache lookup\nHot: moka in-memory\nCold: SQLite]
    policy -->|Yes: --override-media-cache| skip_cache[Skip cache — force recompute]

    lookup --> hit{Cache hit?}
    hit -->|Yes| inject_cached[Inject cached result\nNo worker IPC needed]
    hit -->|No| miss[Send to Python worker]

    skip_cache --> miss
    miss --> infer[Worker returns raw ML output]
    infer --> cache_put[Store result in cache\nKey includes engine_version]
    cache_put --> inject_fresh[Inject fresh result into AST]

    inject_cached --> done([Continue pipeline])
    inject_fresh --> done
```

---

## Cross-Cutting: Incremental Processing (--before)

Supported by `morphotag` and `align`. Compares old vs new CHAT to skip
unchanged content.

```mermaid
flowchart TD
    start([--before provided]) --> read_before[Read before file]
    read_before --> diff[diff_chat — classify utterances\nAdded / Removed / Modified / Unchanged]
    diff --> preserve[Preserve stable dependent tiers\nand refresh reusable timing]
    preserve --> filter[Filter: only reprocess\nAdded + Modified content that still needs work]
    filter --> process[Run NLP only where reuse and cache\ncannot satisfy the request]
    process --> merge_results[Merge: preserved results from before\n+ fresh results for changed]
    merge_results --> done([Full output with minimal recomputation])
```
