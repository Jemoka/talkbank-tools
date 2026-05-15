# `whisper_hub` ASR Engine

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## What it is

`whisper_hub` is an ASR engine variant that loads a **community Whisper
fine-tune from the Hugging Face Hub** by model_id. It exists because
stock `openai/whisper-*` checkpoints produce unusable output on some
languages where Rev.AI also fails, and a per-language fine-tune is the
only path to coherent transcription.

`whisper_hub` is parallel to the other ASR engine variants:

| Engine | What it loads | When to use |
|--------|--------------|-------------|
| `rev` *(default)* | Rev.AI cloud API | Languages where Rev.AI quality is good (English, Spanish, most European). |
| `whisper` | Stock `openai/whisper-large-v3` via HF transformers | Languages where stock Whisper handles the acoustic + language combo well. |
| `whisperx` | WhisperX worker (Whisper + FA) | Fine-grained alignment needs. |
| `whisper_oai` | OpenAI Whisper API | Latency-sensitive cloud transcription in supported languages. |
| **`whisper_hub`** | **HF community fine-tune by model_id** | **Languages where both Rev.AI and stock Whisper fail.** |
| `tencent`, `aliyun`, `funaudio` | Cantonese providers | Chinese variants only. |

## Quick start

```bash
# Uses the per-language default model_id resolved from
# python/batchalign/models/resolve.py. For Malayalam that's
# thennal/whisper-medium-ml.
batchalign3 transcribe input/ output/ --lang mal --asr-engine whisper_hub
```

To override the model for a language that already has a default, or to
pick a model for a language we haven't seeded yet:

```bash
batchalign3 transcribe input/ output/ \
  --lang mal --asr-engine whisper_hub \
  --engine-overrides '{"asr":"whisper_hub","model_id":"other/mal-model"}'
```

## Per-language defaults

The per-language default model_id table lives in
`python/batchalign/models/resolve.py`. It is intentionally small and seeded
reactively from empirical evaluation — a language only gets a default
after we've confirmed the chosen fine-tune produces coherent output.

| Language (ISO-639-3) | Default HF model_id | Notes |
|---|---|---|
| `mal` (Malayalam) | `thennal/whisper-medium-ml` | See "Evaluation below." |

Any other language requires passing `--engine-overrides
'{"asr":"whisper_hub","model_id":"..."}'` — the loader raises
`WhisperHubModelNotFoundError` with a specific message telling the user
how to fix it, instead of falling back to a stock Whisper checkpoint
that would silently produce garbage.

## Why a per-language table and not auto-discovery?

HuggingFace lists dozens of Whisper fine-tunes per language. Their
advertised WER is self-reported and wildly inconsistent, their test
sets vary, and some checkpoints (e.g.,
`DrishtiSharma/whisper-large-v2-malayalam`) have a broken
`generation_config` that refuses to load via HF transformers without a
consumer-side workaround. Auto-picking by download count or name
match would ship the first plausible-looking thing to users with no
quality signal.

The table is hand-curated so every default is traceable to an actual
empirical comparison. The escalation path to automated probing lives
in [`revai-language-quality-strategy.md`](revai-language-quality-strategy.md).

## Evaluation behind the Malayalam seed

A 73-second Malayalam sample was transcribed by four candidates:

| Model | Malayalam-script chars | Repetition | Usable |
|---|---:|---:|---|
| `thennal/whisper-medium-ml` | **100.0 %** | 0.01 | **Yes** |
| `kavyamanohar/whisper-small-malayalam` | 100.0 % | 0.04 | Yes, noisier |
| `openai/whisper-large-v3` | 27.5 % | 0.27 | No (hallucinates "Thank you for watching.") |
| `openai/whisper-medium` | 13.4 % | 0.73 | No (Khmer + Gurmukhi character loops) |

Rev.AI on the same file returned 55 tokens of Hangul + Gurmukhi + Latin
+ U+FFFD — zero Malayalam script. That result drove the deny-list
entry in `revai/preflight.rs::REVAI_KNOWN_BROKEN`, which now
recommends `whisper_hub` for Malayalam specifically.

Artifacts live in an operational workspace outside this public repo.

**Caveats:**

- Single audio file. A native Malayalam reader should compare word-level
  accuracy before shipping this default for a large corpus.
- CPU inference (no GPU numbers). `thennal/whisper-medium-ml` took 353
  seconds on ming's CPU for 73 seconds of audio (4.8× real-time slower).
  GPU should be ~3–5× faster than real-time.

## Fine-tune gotchas

HF Whisper fine-tunes differ from stock OpenAI checkpoints in one
critical way: they bake `language` and `task` into their own
`generation_config`. Passing them again via `generate_kwargs` produces
gibberish — the model applies two competing prompts.

`whisper_hub` handles this by passing `language="auto"` through to the
shared `load_whisper_asr()`, which makes the handle's `gen_kwargs("auto")`
branch fire and omit those overrides. Do not replicate the
`language=<concrete>` path used by stock Whisper when adding a new
fine-tune loader.

## When this engine stops being a fit

If the per-language table grows past ~10 entries and we're re-testing
defaults often, it's time to escalate to a probe harness — see Option C
in [`revai-language-quality-strategy.md`](revai-language-quality-strategy.md).

## Cross-references

- Command flag wiring: [`cli-reference.md`](../user-guide/cli-reference.md)
- Language code mapping: [`language-code-resolution.md`](language-code-resolution.md)
- Rev.AI deny-list strategy (why this engine exists): [`revai-language-quality-strategy.md`](revai-language-quality-strategy.md)
- How to add a new engine variant: [`../developer/adding-engines.md`](../developer/adding-engines.md)
- Stock Whisper page (baseline behavior): [`whisper-asr.md`](whisper-asr.md)
