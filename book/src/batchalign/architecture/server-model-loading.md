# Server Model Loading and Caching

**Status:** Current
**Last updated:** 2026-06-01 00:51 PDT

This document describes every ML model loaded by batchalign3 workers,
when each model is loaded into memory, and how results are cached.

## How Models Are Loaded

The Rust server spawns Python worker processes keyed by `(target, lang)`.
Targets are either:

- a released infer task such as `infer:morphosyntax` or `infer:asr`
- or a test-echo worker bootstrapped for one infer task without loading models

Each worker loads its own models on first use. Workers are managed
by a **WorkerPool** (`crates/batchalign/src/worker/pool/`) with a
configurable idle timeout (default 10 minutes) and automatic crash
restart.

**Warmup** (`warmup: [morphotag, align]` in `server.yaml`) pre-spawns workers
at startup so the first request does not pay the model-loading cost.

---

## Per-Command Model Inventory

### `morphotag` (task string: `morphosyntax`)

| Module | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/morphosyntax.py` | Stanza pipeline (tokenize, pos, lemma, depparse, mwt) | stanza / HF Hub | 300-500 MB per language | First file for each language (lazy per-language dict) | Yes |

**Internal caching:** Per-language `stanza.Pipeline` dict in the worker state.
A single worker handles all languages without reloading.

**Result caching:** SQLite utterance cache. Key = `BLAKE3(words + lang + "|mwt")`,
gated by Stanza version. Stores final `%mor`/`%gra` strings.

---

### `align` (task string: `fa`)

The server auto-chains forced alignment + UTR + disfluency + retrace.

| Module | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/fa.py` (Whisper FA) | `openai/whisper-large-v2` | HF Hub | ~3 GB | Worker startup (immediate) | Yes |
| `inference/asr.py` (UTR) | `talkbank/CHATWhisper-en-large-v1` (eng) or `openai/whisper-large-v2` (other) | HF Hub | ~3 GB | First audio file (lazy) | Yes |
| Rust (disfluency) | None (rule-based data files) | local | negligible | N/A | No |
| Rust (retrace) | None (Rust n-gram) | local | negligible | N/A | No |

**Alternative:** Wave2Vec FA (`inference/fa.py`) uses `torchaudio.pipelines.MMS_FA`
(~1.6 GB, loaded at startup, from PyTorch Hub, not HF Hub).

**Result caching:**
- Forced alignment: SQLite. Key = `BLAKE3(audio_chunk + text + pauses)`.
- UTR: SQLite. Key = `BLAKE3(realpath + filesize)`. Protected from pruning.

---

### `transcribe` (server-owned composition over `asr`)

Current CLI default engine is Rev.AI. Alternate ASR engines are selected with
`--engine whisper`, `--engine chatwhisper`, `--engine openai`,
`--engine funaudio`, `--engine tencent`, or `--engine qwen3` (the legacy
`whisperx` engine has been removed). The server auto-chains disfluency + retrace.
For languages with a dedicated utterance model (`eng`, `cmn`, `zho`, `yue`),
transcribe also runs pre-CHAT utterance segmentation before CHAT assembly.

| Module / Engine | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| Rust `crates/batchalign/src/revai/asr.rs` — Rev (default) | Rev.AI HTTP client only | local + remote API | negligible local memory | per-file server dispatch | No |
| `inference/asr.py` — Whisper OAI | None (OpenAI API) | remote | N/A | N/A | No |
| `inference/asr.py` — Whisper | `openai/whisper-large-v3` + optional BertUtteranceModel | HF Hub | ~3 GB + ~400 MB | Worker startup (immediate) | Yes |
| Rust (disfluency) | None | local | negligible | N/A | No |
| Rust (retrace) | None | local | negligible | N/A | No |

**BertUtteranceModel languages:** Only loaded when `resolve("utterance", lang)`
returns a model name. Currently: `eng` (`talkbank/CHATUtterance-en`),
`cmn` / `zho` (`talkbank/CHATUtterance-zh_CN`), `yue` (Cantonese-specific
model).

**Result caching:** ASR results are NOT cached (each run re-transcribes).

---

### `transcribe_s` (server-owned composition over `asr`)

`transcribe_s` now follows the same server-owned transcribe pipeline as
`transcribe`. As in batchalign2, the user-facing surface is diarized
transcription rather than a standalone `speaker` command. When the selected ASR
backend already returns usable speaker labels (for example Rev.AI or the
Cantonese provider adapters), Rust keeps those labels on the default path. When
`--diarize` is explicitly requested, Rust also composes the low-level `speaker`
infer task as a post-ASR relabeling stage, receives raw diarization segments,
and rewrites speaker codes plus `@Participants` / `@ID` headers through
`batchalign::speaker` even on top of Rev-labeled output.

The default dedicated diarization backend remains Pyannote, matching
batchalign2. Pyannote now loads lazily on the first speaker request in a worker
process and is then reused within that process instead of being rebuilt for
every file.

**Result caching:** Diarization results are NOT cached.

---

### `translate` (task string: `translate`)

| Module / Backend | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/translate.py` — Google (default) | None (Google Translate API) | remote | N/A | N/A | No |
| `inference/translate.py` — Seamless | `facebook/hf-seamless-m4t-medium` | HF Hub | ~1.2 GB | Worker startup (immediate) | Yes |

**Result caching:** SQLite utterance cache. Key = `BLAKE3(text + src_lang + tgt_lang)`.

---

### `utseg` (task string: `utterance`)

| Module | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/utseg.py` | `BertUtteranceModel` for `eng` / `cmn` / `zho` / `yue`; otherwise Stanza pipeline (tokenize, pos, lemma, constituency) | HF Hub / stanza | ~400 MB for BERT model or 300-500 MB per Stanza language | First batch (lazy factory) | Yes |

**Result caching:** SQLite utterance cache. Key = `BLAKE3(text + lang)`.

---

### `coref` (task string: `coref`)

| Module | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/coref.py` | Stanza tokenizer + `ontonotes-singletons_roberta-large-lora` | stanza / HF Hub | ~500 MB | First file (lazy) | Yes |

English only. **Result caching:** None.

---

### `benchmark` (server-owned composition over `asr`)

Same engines as `transcribe` plus:

| Module | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/benchmark.py` | None (Rust DP alignment via `batchalign_core.wer_compute()`) | local | negligible | N/A | No |

---

### `opensmile` (task string: `opensmile`)

| Module | Model | Source | Size | Loaded When | HF Hub |
|--------|-------|--------|------|-------------|--------|
| `inference/opensmile.py` | None (C++ feature extraction) | local | negligible | Worker startup | No |

Feature sets: `eGeMAPSv02`, `GeMAPSv01b`, `ComParE_2016`, `eGeMAPSv01b`.

**Result caching:** None (produces CSV output).

---

## Device Placement

All torch-based inference modules auto-detect the compute device at load time:

| Priority | Device | Notes |
|----------|--------|-------|
| 1 | CUDA | If `torch.cuda.is_available()` and not `--force-cpu` |
| 2 | MPS | macOS Metal (Apple Silicon). Used on Apple Silicon server/client machines when available |
| 3 | CPU | Fallback |

Stanza manages its own device internally (typically CPU).

## HuggingFace Hub Token

Most HF-hosted models used by batchalign3 are public, but the current speaker
diarization stack may depend on gated pyannote assets underneath
`talkbank/dia-fork`. For diarization-capable machines, authenticate Hugging
Face once before the first run and keep a read token available to the worker
runtime.

```bash
# Interactive login (stores token in Hugging Face's local auth cache/keychain)
hf auth login

# Or export a token explicitly for the current shell / worker runtime
export HF_TOKEN="hf_..."  # from https://huggingface.co/settings/tokens
```

If the model page asks you to accept terms, do that once in the browser before
retrying diarization.

Unlike Rev.AI, this is **not yet a Rust-owned credential path**. batchalign3
currently relies on ambient Hugging Face authentication in the CLI/server
process environment: the local Hugging Face auth cache/keychain created by
`hf auth login`, or an explicit `HF_TOKEN` exported where the worker runtime can
see it.

Once a model is downloaded, it is cached on disk at `~/.cache/huggingface/`
and does not re-download on subsequent loads.
