# talkbank-tools

**Status:** Current
**Last updated:** 2026-06-01 00:52 PDT

[![Bazel build/test](https://github.com/TalkBank/talkbank-tools/actions/workflows/bazel-build-all.yml/badge.svg)](https://github.com/TalkBank/talkbank-tools/actions/workflows/bazel-build-all.yml)
[![Batchalign Python](https://github.com/TalkBank/talkbank-tools/actions/workflows/bazel-python.yml/badge.svg)](https://github.com/TalkBank/talkbank-tools/actions/workflows/bazel-python.yml)
[![License: BSD-3-Clause](https://img.shields.io/badge/License-BSD_3--Clause-blue.svg)](LICENSE)

The unified home for the [TalkBank](https://talkbank.org/) toolchain.
Four user-facing products ship from this repo, all centered on the
CHAT transcript format:

- **Batchalign3** — audio/ML pipeline that produces and enriches CHAT
  (transcribe, align, morphotag, translate, segment).
- **`chatter`** — CHAT-first command-line tool for validation,
  normalization, conversion, and CLAN-compatible analysis.
- **VS Code extension** — interactive editor for CHAT files with
  live validation and rich navigation.
- **CLAN command reference** — a Rust reimplementation of CLAN's
  analysis, transform, and converter commands, accessible through
  `chatter clan ...`.

Plus the supporting library surface: the public Rust crates for
parsing/validation/transform, the `chatter-lsp` language server,
the `tree-sitter-talkbank` grammar, the Batchalign Python package,
the PyO3 bridge, and two experimental desktop apps.

## Start here

Pick the row that matches what you want to do today.

| I want to... | Start here |
|---|---|
| Transcribe, align, morphotag, translate, or segment media/transcripts | [Batchalign3 Quickstart](book/src/quickstart/index.md), [Batchalign3 Installation](book/src/batchalign/user-guide/installation.md), [Batchalign3 CLI Reference](book/src/batchalign/user-guide/cli-reference.md) |
| Validate, normalize, convert, or analyze CHAT from the command line | [`chatter` Installation](book/src/chatter/user-guide/installation.md), [`chatter` CLI Reference](book/src/chatter/user-guide/cli-reference.md), [Migrating from CLAN](book/src/chatter/user-guide/migrating-from-clan.md) |
| Edit CHAT files interactively with live validation | [VS Code extension](book/src/vscode/getting-started/installation.md) |
| Look up a specific CLAN command (FREQ, MLU, KIDEVAL, …) | [CLAN command reference](book/src/clan-reference/introduction.md) |
| Integrate CHAT support into another editor | [`chatter-lsp` README](crates/chatter/chatter-lsp/README.md) |
| Work with the typed Rust APIs directly | [Library Usage](book/src/chatter/integrating/library-usage.md) |
| Understand the CHAT grammar/spec/parser pipeline | [chatter Architecture](book/src/architecture/overview.md), [`grammar/`](grammar/), [`resources/spec/`](resources/spec/) |
| Contribute to the repo | [CONTRIBUTING.md](CONTRIBUTING.md) + [Contributing Setup](book/src/contributing/setup.md) |

## Main products

### Batchalign3 — audio + ML CHAT pipeline

The audio/ML-facing CLI for transcription, forced alignment,
morphotagging, utterance segmentation, translation, and benchmarking.

```bash
batchalign3 transcribe recordings/ -o transcripts/ --lang eng
batchalign3 align corpus/ -o aligned/
batchalign3 morphotag corpus/ -o tagged/
```

Public preview product line on the `0.1.x` release. The canonical
public install path is PyPI via `uv`:

```bash
uv tool install batchalign3
```

See the [Batchalign3 Installation guide](book/src/batchalign/user-guide/installation.md)
and [Quickstart](book/src/batchalign/user-guide/quick-start.md) for
first-run details.

### `chatter` — CHAT-first CLI

The command-line tool for everything CHAT-text:

- validation
- normalization / linting
- CHAT ↔ JSON conversion
- CLAN-compatible analysis, transforms, and format converters

```bash
chatter validate transcript.cha
chatter clan freq corpus/
chatter to-json transcript.cha -o transcript.json
```

See the [`chatter` Installation guide](book/src/chatter/user-guide/installation.md),
[CLI reference](book/src/chatter/user-guide/cli-reference.md), and
[CLAN command reference](book/src/clan-reference/introduction.md).

### VS Code extension — interactive CHAT editor

Live syntax highlighting, validation, code-completion, cross-tier
navigation, dependency graphs, and review/coder workflows. VSIX
bundles ship from GitHub Releases.

See [apps/vscode-extension/README.md](apps/vscode-extension/README.md) and the
[VS Code extension Getting Started](book/src/vscode/getting-started/installation.md).

## Surface status at a glance

| Surface | What first-time users should assume today |
|---|---|
| Batchalign3 CLI / server / dashboard | Public preview Batchalign product surface for audio/ML workflows |
| `chatter` CLI | Stable public CHAT-first command-line surface; strongest current support story |
| Public Rust core crates | Stable public source-level integration surface for CHAT parsing/validation |
| `chatter-lsp` + VS Code extension | Public preview editor surface; GitHub Releases publish platform-specific VSIX bundles |
| `tree-sitter-talkbank` grammar | Public preview reusable grammar surface |
| `apps/batchalign/batchalign-gui/` (Batchalign Desktop) | Experimental Batchalign GUI shell only; not a supported release surface |
| `apps/chatter/chatter-gui/` (Chatter Desktop) | Experimental validation GUI only; not a supported release surface |

## Documentation

All user, developer, and architecture docs live in **one** mdBook:
[`book/`](book/) — the **TalkBank Toolchain** book. It absorbs four
previously-separate sub-books (chatter, Batchalign3, VS Code,
CLAN command reference) into one tree organized by audience-first
sections under `book/src/`.

| Section | What lives there | Entry point |
|---|---|---|
| Front matter | What is this, install hub, choose-your-path quickstart | [Introduction](book/src/introduction.md), [Install](book/src/install/index.md), [Quickstart](book/src/quickstart/index.md) |
| `book/src/batchalign/` | Batchalign3: migration from BA2, user guide, architecture, technical reference, developer guide, design decisions | [Batchalign3 introduction](book/src/batchalign/introduction.md) |
| `book/src/chatter/` | `chatter` CLI: user guide, integration | [`chatter` introduction (book root)](book/src/introduction.md) |
| `book/src/chat-format/` | The CHAT format reference: headers, utterances, retraces, replacements, dependent tiers, symbols | [CHAT format overview](book/src/chat-format/overview.md) |
| `book/src/vscode/` | VS Code extension: getting started, editing, navigation, media, analysis, review, coder, workflows, configuration, troubleshooting, developer guide | [VS Code Getting Started](book/src/vscode/getting-started/installation.md) |
| `book/src/clan-reference/` | CLAN command reference: per-command pages for the analysis, transform, and converter families | [CLAN command reference introduction](book/src/clan-reference/introduction.md) |
| `book/src/architecture/` | Architecture and parser/grammar/data-model design | [Architecture overview](book/src/architecture/overview.md) |
| `book/src/contributing/` | Contributor workflows, testing, coding standards, dev checks | [Contributing Setup](book/src/contributing/setup.md) |

Build the book locally:

```bash
just docs build
just docs serve   # opens http://localhost:3000
```

Repo-level release-contract policy documents live under
[`book/src/operations/`](book/src/operations/) — platform support
matrix, release contract, versioning policy, and the auto-generated
error catalog under `book/src/operations/errors/`.

## Repository map

| Path | What lives there |
|---|---|
| `book/` | The unified TalkBank Toolchain mdBook (all four product surfaces, CHAT format, architecture, contributing) |
| `crates/` | Rust crates: parser, model, transform, CLAN, CLI, LSP, plus the `batchalign-*` crates |
| `python/batchalign/` | Python package for `batchalign3` |
| `crates/batchalign/batchalign-engine/` | PyO3 cdylib backing the wheel |
| `apps/chatter/chatter-gui/` | Tauri validation app (experimental) |
| `apps/batchalign/batchalign-gui/` | Tauri Batchalign desktop GUI (experimental) |
| `apps/vscode-extension/` | VS Code extension source |
| `grammar/` | Tree-sitter grammar |
| `resources/spec/` | Spec source of truth |
| `crates/spec/` | Spec generator crates |
| `schemas/` | JSON Schema artifacts |

## Installing and building

### Install `chatter` / `chatter-lsp` from source

```bash
cargo install --path crates/chatter/chatter-cli
cargo install --path crates/chatter/chatter-lsp
```

Or via Bazel (the canonical build path):

```bash
just chatter build       # builds CLI + LSP into bazel-bin/
```

### Install `batchalign3`

```bash
uv tool install batchalign3
```

Repo-hosted `.command`/`.bat` helper scripts under
[`installers/`](scripts/installers/README.md) wrap the same `uv tool install
batchalign3` flow; they are not a separate signed installer channel.

### Common developer commands

```bash
just --list                                # overview of repo-native tasks
just build                                 # bazel build //... (release)
just test                                  # bazel test //...  (release)
bazel build //... && bazel test //...      # canonical pre-merge gate
bazel build //crates/batchalign/...        # Batchalign Rust compile checks
bazel test //crates/batchalign/...         # Batchalign Rust test suites
just batchalign wheel                      # host-platform wheel
just docs build                            # build the unified TalkBank Toolchain mdBook
```

For lower-level build helpers:

```bash
cargo run -q -p xtask -- help
```

## License

BSD-3-Clause. Copyright (c) 2026, Carnegie Mellon University. See
[LICENSE](LICENSE).
