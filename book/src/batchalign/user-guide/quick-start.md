# Quick Start

**Status:** Current
**Last updated:** 2026-05-11 11:42 EDT

This chapter covers the most common `batchalign3` workflows from the terminal.
The examples assume the `batchalign3` binary is installed and that local
processing commands can reach a Python runtime with `batchalign.worker`
available.

**Evaluating the experimental GUI shell?** See
[Batchalign Desktop (Experimental)](desktop-app.md). The supported first-time
user path today is still the `batchalign3` CLI.

For the full command surface, see the [CLI Reference](cli-reference.md).

## Before you start

**Model downloads:** The first time you run a processing command, Batchalign
downloads ML models (~2 GB). This is a one-time cost — subsequent runs use
cached models from disk.

**Caching:** Batchalign caches **audio-bound** intermediate results
(forced-alignment word timings and the UTR ASR pass) in a local SQLite
database, so re-running `align` or `transcribe` on the same audio
returns those steps from cache. Text-NLP commands (`morphotag`,
`utseg`, `translate`, `coref`) are not cached and always recompute.
See [Caching](caching.md) for details.

**Performance:** Back-to-back runs are still much faster than first-run model
downloads because models and caches stay on disk. If you need hot in-memory
workers across repeated runs, start an explicit server with `batchalign3 serve`.
See [Performance](performance.md) for tuning tips.

## Basic command shape

```bash
batchalign3 [GLOBAL OPTIONS] COMMAND [COMMAND OPTIONS] [PATHS...]
```

- Global options go before the command.
- Most processing commands use `-o/--output` for a destination directory.
- Omitting `-o/--output` means in-place processing when the command supports it.

## Transcribe audio to CHAT

```bash
batchalign3 transcribe ~/recordings/ -o ~/transcripts/ --lang eng
```

To use OpenAI Whisper instead of the default Rev.AI engine:

```bash
batchalign3 transcribe ~/recordings/ -o ~/transcripts/ \
  --asr-engine whisper-oai --lang eng
```

To use a local Whisper model:

```bash
batchalign3 transcribe ~/recordings/ -o ~/transcripts/ \
  --asr-engine whisper --lang eng
```

Important routing note: explicit `--server` now submits shared-filesystem
`paths_mode` jobs for `transcribe`. The target server must be able to read the
same input paths and write the requested output paths.

## Align transcripts against audio

```bash
batchalign3 align ~/corpus/ -o ~/aligned/
```

Common useful flags:

```bash
batchalign3 align ~/corpus/ -o ~/aligned/ --wor
batchalign3 align ~/corpus/ -o ~/aligned/ --fa-engine whisper
batchalign3 align ~/corpus/ -o ~/aligned/ --utr-engine whisper
```

## Add morphosyntactic analysis

```bash
batchalign3 morphotag ~/corpus/ -o ~/tagged/
```

Useful variants:

```bash
batchalign3 morphotag ~/corpus/ -o ~/tagged/ --retokenize
batchalign3 morphotag ~/corpus/ -o ~/tagged/ --skipmultilang
```

`morphotag` is not cached, so repeated runs run the full Stanza pipeline
again. The wall-clock win for repeated runs comes from keeping workers
warm in memory rather than from disk caching. For interactive sessions
where you want workers to stay loaded across commands, use explicit
server mode (`batchalign3 serve start` plus `--server`).

## Verbosity

```bash
batchalign3 align ~/corpus/ -o ~/aligned/
batchalign3 -v align ~/corpus/ -o ~/aligned/
batchalign3 -vv align ~/corpus/ -o ~/aligned/
batchalign3 -vvv align ~/corpus/ -o ~/aligned/
```

## Run logs

```bash
batchalign3 logs
batchalign3 logs --last
batchalign3 logs --export
batchalign3 logs --clear
```

## Remote server mode

For commands that support explicit remote dispatch:

```bash
batchalign3 --server http://yourserver:8000 morphotag ~/corpus/ -o ~/tagged/
batchalign3 --server http://yourserver:8000 align ~/corpus/ -o ~/aligned/
```

the local daemon and ignore `--server` (see `command_prefers_local_daemon`
in `crates/batchalign/src/cli/dispatch/mod.rs`). The remaining text and
analysis commands (`morphotag`, `align`, `compare`, etc.) honor explicit
`--server` routing.

## Next steps

- [Batchalign Desktop (Experimental)](desktop-app.md) — in-repo GUI shell status and scope
- [CLI Reference](cli-reference.md)
- [Performance](performance.md)
- [Server Mode](server-mode.md)
- [Rev.AI Integration](rev-ai.md)
- [Python API](python-api.md)
