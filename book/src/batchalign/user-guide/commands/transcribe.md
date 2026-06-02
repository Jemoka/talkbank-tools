# transcribe

**Status:** Current
**Last updated:** 2026-06-01 00:51 PDT

Create a new CHAT transcript from audio files using automatic speech
recognition (ASR). Produces `.cha` files alongside or in a separate output
directory. **Never modifies the input audio.**

---

## Quick start

```bash
# Transcribe a single recording, output alongside input
batchalign3 transcribe interview.wav

# Transcribe all audio files in a directory
batchalign3 transcribe recordings/ -o transcripts/ --lang eng

# Auto-detect language (useful for bilingual/code-switched audio)
batchalign3 transcribe bilingual.wav -o out/ --lang auto

# Transcribe with dedicated speaker diarization (Whisper, multiple speakers)
batchalign3 transcribe interview.wav -o out/ --engine whisper --diarization enabled

# Use the remote server
batchalign3 --server http://your-server:8001 transcribe corpus/ -o out/ --lang eng
```

If you enable dedicated speaker diarization for the first time on a machine,
authenticate Hugging Face first:

```bash
hf auth login
```

You may also need to accept the diarization model's terms in the browser once
before retrying the command.

At the moment this uses **ambient Hugging Face auth**, not the Rust-owned
provider-credential path used for Rev.AI. In practice that means the machine
running `batchalign3` must already have a valid `hf auth login` cache/keychain
entry, or an `HF_TOKEN` exported in that process environment.

---

## Pipeline

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
    engine_check -->|rev| rev_preflight["Rev.AI preflight\nPre-submit audio in parallel\nskip_postprocessing=true for en/es"]
    engine_check -->|openai| whisper_oai["OpenAI Whisper API\n(turbo)"]
    engine_check -->|funaudio| funaudio["FunAudio SenseVoice\n(Cantonese/Mandarin)"]
    engine_check -->|tencent| tencent["Tencent Cloud ASR"]
    engine_check -->|qwen3| qwen3["Qwen3-ASR\n(Alibaba open-weight)"]

    rev_preflight --> rev_poll[Poll Rev.AI for results]
    rev_poll --> asr_tokens

    whisper --> asr_tokens
    chatwhisper --> asr_tokens
    whisper_oai --> asr_tokens
    funaudio --> asr_tokens
    tencent --> asr_tokens
    qwen3 --> asr_tokens

    asr_tokens["Raw ASR tokens\nword + start_s + end_s + optional speaker + confidence"]
    asr_tokens --> convert["convert_asr_response()\nGroups tokens by speaker label"]
    convert --> dedicated_check{"--diarization enabled?"}
    dedicated_check -->|No| postprocess
    dedicated_check -->|Yes| speaker_v2["execute_v2(task=speaker)\nprepared audio → diarization segments\nPost-ASR relabeling via Pyannote or NeMo"]
    speaker_v2 --> postprocess

    subgraph postprocess ["Rust post-processing: process_raw_asr()"]
        direction TB
        p1[1. Compound merging] --> p2[2. Multi-word splitting\nsplit tokens with spaces, interpolate timestamps]
        p2 --> p3[3. Number expansion\ndigits → word form]
        p3 --> p3check{lang=yue?}
        p3check -->|Yes| p4["4. Cantonese normalization\nOpenCC + domain replacements"]
        p3check -->|No| p5
        p4 --> p5[5. Long-turn splitting\nchunk at >300 words]
        p5 --> p6[6. Retokenization\npunctuation-based utterance splitting]
        p6 --> p7[7. Disfluency replacement\nfilled pauses + orthographic from per-language wordlists]
        p7 --> p8[8. N-gram retrace detection\nwrap repeated n-grams in `&lt;...&gt; [/]`]
    end

    postprocess --> build_chat["build_chat → ChatFile AST\nHeaders, participants, %wor tiers"]
    build_chat --> speaker_apply{Dedicated speaker\nsegments present?}
    speaker_apply -->|Yes| reassign["reassign_speakers()\nRewrite utterance speakers +\n@Participants + @ID headers"]
    speaker_apply -->|No| utseg_check{"with_utseg?\ndefault: true"}
    reassign --> utseg_check

    utseg_check -->|Yes| run_utseg[process_utseg\nBERT-based re-segmentation]
    utseg_check -->|No| mor_check{"with_morphosyntax?\ndefault: false"}

    run_utseg --> mor_check
    mor_check -->|Yes| run_mor[process_morphosyntax\nPOS + lemma + depparse]
    mor_check -->|No| merge_check

    run_mor --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| output

    merge --> output[Serialize → .cha output]
    output --> done([Output .cha file])
```

---

## Utterance boundary detection

`transcribe` always does utterance splitting before CHAT output is written.
There are two paths:

- `eng`, `cmn`, `zho`, `yue` — dedicated pre-CHAT utterance models
- all other languages — punctuation-based splitting in Rust

Standalone `utseg` is the follow-up command for re-segmenting already-built
CHAT transcripts.

---

## Options

### Path options

| Option | Meaning |
| --- | --- |
| `PATHS...` | Audio files (`.mp3`, `.mp4`, `.wav`) or directories |
| `-o`, `--output DIR` | Output directory for new `.cha` files |
| `--file-list FILE` | Read input paths from a text file |
| `--in-place` | Write `.cha` files alongside the audio inputs |

### ASR and language options

| Option | Default | Meaning |
| --- | --- | --- |
| `--lang CODE` | `eng` | 3-letter ISO language code, or `auto` for language auto-detection |
| `--engine {rev,whisper,chatwhisper,openai,funaudio,tencent,qwen3}` | `rev` | ASR engine. Whisper-family engines `whisperx` and `whisper_hub` were removed; `whisper` is HF transformers, `chatwhisper` is the TalkBank fine-tune, and `openai` is the OpenAI Whisper Python library (turbo). |
| `-n`, `--num-speakers N` | `2` | Expected number of speakers |
| `--diarization {auto,enabled,disabled}` | `auto` | Dedicated Pyannote speaker diarization stage (`auto` = disabled) |
| `--wor` / `--nowor` | `--nowor` | Include or suppress the `%wor` word-timing tier |
| `--merge-abbrev` | off | Merge abbreviations in the output |

---

## Speaker labeling: Rev.AI vs Whisper

This is the most common source of confusion with `transcribe`.

**Rev.AI (default engine):** Rev.AI returns speaker labels as part of its ASR
response. These labels are **always** applied — you get multi-speaker output
without passing `--diarization enabled`. Passing `--diarization enabled`
explicitly runs an additional Pyannote post-ASR relabeling stage on top of the
Rev labels, matching BA2's audited `transcribe_s` pipeline behavior.

**Whisper-based engines** (`--engine whisper`, `chatwhisper`, `openai`):
these engines produce no speaker labels. Without `--diarization enabled`, all
utterances are attributed to a single default speaker. Pass
`--diarization enabled` to run a dedicated Pyannote stage that assigns speaker
identities.

**`--diarization auto`** (the default) = disabled dedicated stage. Equivalent
to BA2's `--nodiarize`. The BA2 help text claiming Rev.AI ignored `--diarize`
was stale — the actual BA2 `transcribe_s` pipeline wiring ran the dedicated
stage.

---

## `--lang auto` behavior

With `--engine whisper`, `--lang auto` omits the language parameter from
Whisper's generation kwargs, letting the model detect the spoken language from
the audio. The multilingual `openai/whisper-large-v3` model is always used
with `auto` — language-specific fine-tuned models are bypassed because they
are trained for a single language.

With Rev.AI, `--lang auto` submits a true auto-language request to the Rev.AI
API. Note that Rev.AI auto-detect and explicit `--lang eng` can produce
different punctuation, diarization, and turn boundaries from the provider.

---

## What gets created

A new `.cha` file per audio input (audio extension replaced: `foo.wav` →
`foo.cha`). Contains:

- `@Comment` with Batchalign version and ASR engine name
- `@Languages`, `@Participants`, `@ID` headers
- Utterance lines with timing bullets
- `%wor` tier (if `--wor` is set)

No `%mor` or `%gra` tiers are created by `transcribe`. Run `morphotag`
afterwards if morphosyntactic analysis is needed.

---

## Gotchas

**Rev.AI `skip_postprocessing`:** For English and Spanish (the only
languages Rev.AI's API documents the parameter as supporting), Rev.AI
is called with `skip_postprocessing=true`. The hint table is in
`crates/batchalign/src/revai/preflight.rs::skip_postprocessing_hint`,
which matches Rev.AI's own 2-letter codes `"en" | "es"` and returns
`None` for everything else. The flag is true because CHAT records
spoken form (`"eighty percent"`, `"seventeen year old"`); leaving it
off causes Rev.AI to apply ITN and return main-tier-illegal forms
like `"80%"` / `"17-year-old"`. For languages outside the en/es
support pair, no flag is sent (the parameter is a no-op there per
Rev.AI's docs), and BA3's downstream post-processing handles
spoken-form normalization.

**`--server` requires server-visible audio.** With `--server`, the server
resolves audio paths on its own filesystem. Paths valid on your machine must
also be reachable from the server, or you must use a shared media mount.

**Memory on developer machines.** Each Whisper model instance uses 2–15 GB.
For large corpus runs (more than a handful of files or >1 GB audio total),
prefer a dedicated server with substantial RAM (via `--server`) over a
developer laptop, and always pass `--workers 1` for local smoke tests.

---

## Related documentation

- [Rev.AI Integration](../rev-ai.md) — API key setup, engine behavior
- [Cantonese Engines](../cantonese-processing.md) — Tencent, Aliyun, FunASR engines
- [Utterance Segmentation](../../reference/utterance-segmentation.md) — post-ASR BERT utseg
- [Command I/O: transcribe](../../reference/command-io.md#2-transcribe) — I/O patterns and mutation behavior
- [Command Flowcharts: transcribe](../../architecture/command-flowcharts.md#transcribe) — full architecture flowchart
- [ASR Token Pipeline](../../architecture/asr-token-pipeline.md) — ASR post-processing details
