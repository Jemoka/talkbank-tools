# batchalign

TalkBank CHAT processing pipeline — ASR, forced alignment, morphosyntax
(`%mor` / `%gra`), utterance segmentation, translation, and compare.
The user-facing Python package; the runtime is a PyO3 extension backed
by the Rust crates in `crates/batchalign/`.

User and developer docs live in `book/src/batchalign/` (the mdBook is
the source of truth). See `book/src/batchalign/developer/building.md`
for the canonical build recipe.

## Install

From PyPI (stable wheels):

```bash
pip install batchalign
```

From source: **use the `just` recipes** — they go through Bazel, so
every dep (proto codegen, Rust crates, PyO3 extension, Python wheel
extras) is materialized for you. Don't reach for `maturin develop` or
`uv sync` directly unless you're debugging the build itself.

```bash
just batchalign build              # build every Bazel target for batchalign
just batchalign test               # run every Bazel test target
just batchalign cli --help         # run `batchalign3` via the development bridge
just batchalign pytest             # pytest (with full Bazel dep graph)
just batchalign wheel              # host-platform wheel at python/target/wheels/
just batchalign sidecar            # standalone daemon binary via PyApp
just batchalign lint               # mypy (+ ruff)
just batchalign versions           # source-of-truth version readout
```

`just --list batchalign` shows the full recipe list. The `just`
recipes call into Bazel, so `tools/bazel` (the bundled wrapper) takes
care of:

- regenerating the pydantic-v2 wire types from
  `crates/batchalign/batchalign-core/src/proto/*.rs`
- rebuilding the `batchalign_core` PyO3 cdylib via maturin under the
  hood
- staging the binary into the wheel
- propagating dependency changes to dependent targets

If you genuinely need `bazel` directly (because a recipe you want
isn't wrapped):

```bash
bazel build //...           # everything
bazel test //...            # everything
bazel run //book:html       # static book HTML
bazel run //apps/batchalign/batchalign-gui:openapi   # GUI OpenAPI codegen
```

The base wheel ships only the lightweight runtime. Heavy ML backends
are gated behind extras — install only what you use:

```bash
pip install 'batchalign[whisper]'      # Whisper ASR
pip install 'batchalign[malayalam]'    # Malayalam Wav2Vec2 XLSR ASR
pip install 'batchalign[stanza]'       # morphosyntax (%mor / %gra)
pip install 'batchalign[pyannote]'     # speaker diarization
pip install 'batchalign[revai]'        # Rev.AI cloud ASR
pip install 'batchalign[google]'       # Gemini cloud ASR + diarization
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
- `book/` — user + developer documentation (mdBook; source of truth).

For repo conventions, build commands, and the BA3 cutover plan, see
`CLAUDE.md` at the repo root and
`book/src/batchalign/developer/landing-status.md`.
