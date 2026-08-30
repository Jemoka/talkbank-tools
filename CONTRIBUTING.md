# Contributing to Batchalign

**Last updated:** 2026-08-30 12:03 EDT

Batchalign combines a typed Rust pipeline, Python model backends, and a desktop
shell. CHAT infrastructure is consumed from the pinned TalkBank Chatter
dependency.

## Setup and commands

Install the toolchains pinned by the repository, then use the Bazel-backed
recipes from the workspace root:

```bash
just batchalign build debug
just batchalign test debug
just batchalign pytest
just batchalign lint
just batchalign cli -- --help
```

The CLI recipe is the supported development entrypoint. It rebuilds the
`//python/batchalign` `py_binary` and restores the caller's working directory so
relative input paths behave normally.

Local Bazel work uses one job by default. Increase `BATCHALIGN_BAZEL_JOBS` only
on a host with enough memory. Do not run Rust/Bazel compilation concurrently
with Stanza, Whisper, or other model-bearing fixture runs.

## Code organization

- `crates/batchalign/batchalign-core` owns pipeline domain types, task runners,
  CHAT-facing transformations, and backend protocols.
- `crates/batchalign/batchalign-engine` owns dispatch, bounded batching, cache,
  pipeline execution, and PyO3 integration.
- `python/batchalign` owns Python backends, recipes, worker processes, and CLI.
- `apps/batchalign` owns the desktop application.

Keep modules conceptual, APIs narrow, and backend submission bounded. Do not
add compatibility namespaces for normal Batchalign behavior: reusable domain
logic belongs in Batchalign core, while orchestration glue belongs in task
runners or the engine.

## Testing changes

Use focused tests while iterating. Before handoff, run the relevant Batchalign
Bazel test scope. Corpus and parity checks should use small chunks with the
task cache cleared between implementation flips. The parity runner under
`scripts/parity` enforces bounded workers and monitors resident memory.

When changing CHAT output, add explicit input and gold fixtures. Production
code must use typed Chatter construction and validation instead of weakening a
validator or assembling tiers as strings.
