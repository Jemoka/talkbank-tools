# Batchalign

**Status:** Current
**Last updated:** 2026-09-06

[Batchalign](https://github.com/TalkBank/batchalign) is TalkBank's audio and
language-processing toolkit for CHAT transcripts. It supports transcription,
forced alignment, morphosyntactic analysis, utterance segmentation,
translation, diarization, comparison, and an experimental desktop shell.

The repository contains the Rust engine under `crates/batchalign/`, the Python
package and CLI under `python/batchalign/`, and the desktop application under
`apps/batchalign/`. CHAT parsing, validation, and typed document structures are
provided by the version-pinned
[Chatter repository](https://github.com/TalkBank/chatter).

## Start here

- New users: [Installation](batchalign/user-guide/installation.md) and
  [Quick Start](batchalign/user-guide/quick-start.md)
- Existing Batchalign users: [Migration Guide](batchalign/migration/index.md)
- Contributors: [Building & Development](batchalign/developer/building.md) and
  [Testing](batchalign/developer/testing.md)
- Integrators: [CLI Reference](batchalign/user-guide/cli-reference.md) and
  [Server Mode](batchalign/user-guide/server-mode.md)

The remainder of this book documents Batchalign behavior, architecture, and
the CHAT-format contracts it relies on. Some historical design material is
retained to explain the migration from the former combined toolchain.

## Repository layout

```text
apps/batchalign/             Batchalign desktop application
crates/batchalign/           Rust core and execution engine
python/batchalign/           Python workers, package, and CLI
resources/test_fixtures/     Integration and parity fixtures
scripts/parity/              Bounded parity tooling
book/                        This documentation
```

The TalkBank Project is supported by NIH grant HD082736.
