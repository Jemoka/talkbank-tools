# batchalign

TalkBank CHAT processing pipeline — ASR, forced alignment, morphosyntax
(`%mor` / `%gra`), utterance segmentation, translation, and compare.
This is the user-facing Python package; the runtime is a PyO3 extension
backed by the Rust crates in `crates/batchalign/`.

See `book/src/batchalign/` for end-user docs and
`book/src/batchalign/developer/building.md` for the canonical build
recipe.

## Install

From PyPI (stable wheels):

```bash
pip install batchalign
```

From source (development):

```bash
cd python
uv sync --group dev
RUSTUP_TOOLCHAIN=1.95.0 uv run maturin develop \
    -m ../crates/batchalign/batchalign-pyo3/Cargo.toml \
    -F pyo3/extension-module
```

The base wheel ships only the lightweight runtime. Heavy ML backends
are gated behind extras — install only what you use:

```bash
pip install 'batchalign[whisper]'      # Whisper ASR
pip install 'batchalign[stanza]'       # morphosyntax (%mor / %gra)
pip install 'batchalign[pyannote]'     # speaker diarization
pip install 'batchalign[revai]'        # Rev.AI cloud ASR
pip install 'batchalign[cantonese]'    # Cantonese pipeline (FunASR, Tencent)
pip install 'batchalign[qwen3]'        # Qwen3 ASR + forced aligner
pip install 'batchalign[nllb]'         # NLLB translation
pip install 'batchalign[api]'          # FastAPI daemon (`batchalign3 daemon`)
pip install 'batchalign[all]'          # everything
```

## CLI

```bash
batchalign3 --help

batchalign3 transcribe input_dir -o output_dir --lang eng
batchalign3 align     input_dir -o output_dir --engine wav2vec
batchalign3 morphotag input_dir -o output_dir --language en
batchalign3 utseg     input_dir -o output_dir
batchalign3 translate input_dir -o output_dir --target eng
batchalign3 compare   input_dir gold_dir   -o output_dir

batchalign3 version                    # banner, version, git SHA
batchalign3 cache {path,stats,clear}   # local result cache
batchalign3 daemon                     # FastAPI server (needs [api])
```

When `-o` is omitted, results are written back in place. The CLI accepts
either a single CHAT/media file or a folder (walked recursively).

## Programmatic API

```python
import batchalign as ba

# Build a backend chain.
pipeline = ba.recipes.morphotag(
    stanza_backend=ba.StanzaBackend(lang="en"),
)

# Or compose your own.
asr = ba.WhisperBackend(language=ba.LanguageCode.from_iso("eng"))
utseg = ba.CHATUtteranceBackend(model="talkbank/CHATUtterance-en")
pipeline = ba.recipes.transcribe(asr_backend=asr, utseg_backend=utseg)

# Run.
inputs = [ba.media_from_path("session.wav")]
outcomes = list(pipeline.run(inputs))
for outcome in outcomes:
    outcome.write("session.cha")
```

## Repository layout

This package is one slice of the `talkbank-tools` monorepo:

- `python/batchalign/` — Python package (this file).
- `crates/batchalign/` — Rust crates (`batchalign-core`, `batchalign-engine`).
- `crates/core/` — shared CHAT parser / model / transform.
- `apps/batchalign/batchalign-gui/` — Tauri desktop GUI.
- `book/` — user + developer documentation.

For repo conventions, build commands, and the BA3 cutover-from-Franklin
plan, see `CLAUDE.md` at the repo root.
