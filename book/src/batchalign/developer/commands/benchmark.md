# benchmark — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 08:18 EDT

Implementation guide for the `benchmark` command. For user-facing
documentation, see [User Guide: benchmark](../../user-guide/commands/benchmark.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `BenchmarkArgs` | asr-engine, lang, num-speakers, wor/nowor |
| Command definition | `crates/batchalign/src/commands/benchmark.rs` | `CommandDefinition` impl |
| Benchmark pipeline | `crates/batchalign/src/runner/dispatch/benchmark_pipeline.rs` | Orchestrates transcribe → compare → materialize |
| Benchmark composition | `crates/batchalign/src/benchmark.rs` — `process_benchmark()` | Calls process_transcribe(), then process_compare_main_annotated() |

---

## Composite architecture

`benchmark` is the canonical `Composite` command. It calls two sub-workflows
in sequence using their shared internal dispatch helpers:

1. `transcribe_pipeline.rs` — produces the hypothesis `ChatFile`
2. `compare.rs` — produces `ComparisonBundle` from hypothesis + gold

The materializer for `benchmark` is `materialize_main_annotated()` function
(injects comparison annotations on the main/hypothesis side), which is the
**opposite** of the released `compare` command's `materialize_released()` function.
They share the same `ComparisonBundle` type but use different output views.

---

## Gold file discovery

Gold files (`FILE.cha`) are expected alongside the audio (`FILE.mp3`) with the
same stem. If the gold file is missing, the audio file is reported as failed
with a typed `GoldFileMissing` error.

---

## Testing

```bash
bazel test //...
# Full ML golden test (ASR + compare — only on net)
cargo nextest run --profile ml -E 'test(benchmark::golden)'
```

---

## Related developer documentation

- [Command Flowcharts: benchmark](../../architecture/command-flowcharts.md#benchmark)
- [compare developer reference](compare.md)
- [transcribe developer reference](transcribe.md)
- [Adding Commands](../adding-commands.md) — use `benchmark` as the reference for `Composite`
