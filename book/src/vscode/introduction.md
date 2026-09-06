# TalkBank CHAT Editor

**Status:** Current
**Last updated:** 2026-03-30 13:40 EDT

The TalkBank CHAT Editor is a VS Code extension for working with
[CHAT format](https://talkbank.org/0info/manuals/CHAT.html) transcripts.
It replaces the legacy CLAN macOS application with a modern, cross-platform
editor backed by a Rust language server.

## What it does

- **Real-time validation** — errors appear as you type, with quick fixes
- **33 CLAN analysis commands** — FREQ, MLU, DSS, KidEval, and more
- **Media playback** — click a bullet to play audio, waveform visualization
- **Transcription mode** — F4 bullet stamping with live audio
- **Review mode** — rate and correct alignment quality with keyboard shortcuts
- **Dependency graphs** — visualize grammatical relations
- **Cross-tier alignment** — click a word, see its morphology and timing
- **Coder mode** — structured annotation with `.cut` code files

## Who this book is for

- **Researchers** using CHAT files for language sample analysis
- **Transcribers** creating and editing CHAT transcripts
- **Students** learning the CHAT format
- **Developers** extending the extension or language server

## How to read this book

- **New to the extension?** Start with [Installation](getting-started/installation.md)
  and [Your First CHAT File](getting-started/first-file.md).
- **Reviewing aligned files?** Go directly to the
  [Review Mode Tutorial](review/tutorial.md).
- **Looking for a specific feature?** Use the sidebar or the
  [Quick Reference](getting-started/quick-reference.md).
- **Building on the extension?** See the [Developer](developer/architecture.md)
  section.

## Requirements

- **VS Code Insiders** (version 1.110 or later)
- **macOS, Windows, or Linux**
- The `chatter` CLI binary (installed automatically from the
  [talkbank-tools](https://github.com/TalkBank/batchalign) release)
