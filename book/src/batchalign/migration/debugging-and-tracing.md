# Debugging and Tracing Migration

**Status:** Current
**Last updated:** 2026-03-19

## BA2 Baseline: No Principled Debugging Story

Batchalign2 (baseline commit `84ad500b`) had no structured debugging infrastructure:

- **Tiered `-v` console logging** (~30 scattered `L.info()` /
  `baL.info()` calls, ~15 raw `print()` statements) via Python's
  `logging` module and Rich console formatting
- **Ephemeral console-only output** — no filesystem dumps, no structured traces,
  no per-stage instrumentation, no debug env vars, no timing breakdowns, no metrics
- Debugging meant: run with `-vvv`, read console, manually inspect I/O files

There were no debug artifacts, no offline replay capability, and no way to
reproduce a pipeline failure without re-running the ML models.

## BA3: Three-Tier Debugging Architecture

BA3 introduces a principled three-tier approach:

### Tier 1: Structured Logging (`tracing` crate)

Already shipped. See [Tracing and Debugging](../developer/tracing-and-debugging.md)
for the `-v`/`-vv`/`-vvv` verbosity system, engine boundary tracing, and
per-component instrumentation.

### Tier 2: `--debug-dir` for Reproducible Filesystem Dumps

The `--debug-dir PATH` CLI flag (or `BATCHALIGN_DEBUG_DIR` env var) enables
structured CHAT/JSON artifact dumps at each pipeline stage. This enables:

- **Offline TDD**: load fixture data, call pipeline functions, assert on output
  without running ML models
- **Test fixture generation**: debug artifacts from real pipeline runs become
  regression test inputs
- **Stage decomposition**: inspect intermediate state between every pipeline
  stage

**Coverage:** debug artifact dumps are wired into both the **align** (FA/UTR)
and **transcribe** (ASR → build CHAT → utseg → morphosyntax) pipelines.

Full directory layout for all artifact types:

```text
debug-dir/
  # ── Transcribe pipeline artifacts ──
  sample_asr_response.json       # Raw ASR tokens + timestamps
  sample_post_asr.cha            # CHAT after assembly (before utseg)
  sample_pre_utseg.cha           # CHAT entering utterance segmentation
  sample_post_utseg.cha          # CHAT after utterance segmentation
  sample_pre_morphosyntax.cha    # CHAT entering morphosyntax

  # ── Align pipeline artifacts ──
  sample_utr_input.cha           # CHAT before UTR injection
  sample_utr_tokens.json         # ASR timing tokens fed to UTR
  sample_utr_output.cha          # CHAT after UTR injection
  sample_utr_result.json         # UTR injection statistics
  sample_fa_input.cha            # CHAT before FA (after UTR)
  sample_fa_grouping.json        # FA group plan (time windows, words)
  sample_fa_group_0.json         # Per-group words + timings
  sample_fa_group_1.json
  sample_fa_output.cha           # Final aligned CHAT
```

### Tier 2b: Always-On Error Logging

Even without `--debug-dir`, certain failure modes automatically log diagnostic
data at `WARN` level — zero cost in the happy path:

| Failure | What is logged |
|---------|---------------|
| Utseg pre-validation fails | Full CHAT text + parse error details |
| Whisper inverted timestamps | Warning with start/end values |
| MOR item count mismatch | Word count + MOR count + utterance text |
| Stanza sentence count mismatch | Expected vs actual counts |

### Tier 3: Dashboard Traces (`debug_traces`)

When `--debug-dir` is specified, `debug_traces` is automatically enabled on job
submissions. The server collects `FaTimelineTrace` data for each file and
exposes it via `GET /jobs/{id}/traces` for dashboard visualization.

## Example Workflow: Reproduce a Transcribe-to-Utseg Failure

```bash
# 1. Run transcription with debug artifacts
batchalign3 transcribe audio/ output/ --lang eng --debug-dir /tmp/ba3-debug

# 2. If utseg fails, inspect the CHAT that was produced
cat /tmp/ba3-debug/sample_post_asr.cha

# 3. Validate it offline to find the exact parse error
cargo run -p chatter-cli -- validate /tmp/ba3-debug/sample_post_asr.cha

# 4. Without --debug-dir, check server logs for the automatic warn! dump
```

## Example Workflow: Reproduce a UTR Failure

```bash
# 1. Run alignment with debug artifacts
batchalign3 align input/ output/ --lang eng --debug-dir /tmp/ba3-debug

# 2. Inspect the UTR input and tokens
cat /tmp/ba3-debug/sample_utr_input.cha
jq . /tmp/ba3-debug/sample_utr_tokens.json

# 3. Write a test that loads the fixtures and calls inject_utr_timing directly
# (no ML model needed — the tokens are already captured)
```

## Fine-Grained Cache Overrides

BA3 also introduces `--override-media-cache-tasks` for per-task cache control:

```bash
# Skip only UTR ASR cache (keep morphosyntax, FA caches)
batchalign3 align input/ output/ --override-media-cache-tasks utr_asr

# Skip UTR + FA caches
batchalign3 align input/ output/ --override-media-cache-tasks utr_asr,forced_alignment
```

Valid task names: `morphosyntax`, `utr_asr`, `forced_alignment`,
`utterance_segmentation`, `translation`.

The existing `--override-media-cache` flag continues to skip all caches.
