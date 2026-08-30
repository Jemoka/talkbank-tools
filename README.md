# Batchalign

**Status:** Current
**Last updated:** 2026-08-30 12:03 EDT

This repository contains Batchalign: the audio and ML pipeline for producing
and enriching CHAT transcripts. It supports transcription, forced alignment,
morphotagging, utterance segmentation, translation, diarization, comparison,
and the Batchalign desktop shell.

The CHAT parser, typed model, transformations, and grammar come from the
version-pinned [TalkBank Chatter repository](https://github.com/TalkBank/chatter).
They are dependencies of Batchalign rather than separately shipped products in
this repository.

## Development

Use the Bazel-backed `just` recipes:

```bash
just batchalign build debug
just batchalign test debug
just batchalign pytest
just batchalign cli -- --help
```

`just batchalign cli` is the authoritative development entrypoint for the CLI.
Bazel jobs default to one to keep local builds memory-safe; set
`BATCHALIGN_BAZEL_JOBS` explicitly on a larger build host if needed.

The main source areas are:

- `crates/batchalign/` — typed Rust core and PyO3 engine
- `python/batchalign/` — Python backends and CLI
- `apps/batchalign/` — desktop application
- `resources/test_fixtures/` — integration and parity fixtures
- `scripts/parity/` — bounded pre/post parity tooling

Install the published CLI with `uv tool install batchalign3`. See
[the Batchalign documentation](book/src/batchalign/introduction.md) for usage
and architecture.

## License

BSD-3-Clause. Copyright (c) 2026, Carnegie Mellon University.
