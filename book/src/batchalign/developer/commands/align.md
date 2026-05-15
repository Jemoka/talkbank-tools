# align — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 11:45 EDT

Implementation guide for the `align` command. For user-facing documentation,
see [User Guide: align](../../user-guide/commands/align.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `AlignArgs` | UTR/FA engine flags, strategy, fuzzy, buffer params |
| Options builder | `crates/batchalign/src/cli/args/options.rs:130–194` (inline dispatch) | Maps `AlignArgs` → `CommandOptions::Align(AlignOptions)` |
| Command definition | `crates/batchalign/src/commands/align.rs` — `AlignCommand` | `CommandDefinition` impl, pre-validation gate |
| FA pipeline | `crates/batchalign/src/runner/dispatch/fa_pipeline.rs` | Per-file FA orchestration: UTR → grouping → FA → injection |
| UTR dispatch | `crates/batchalign/src/runner/dispatch/utr.rs` | `resolve_strategy()`, language-aware strategy gate |
| UTR library | `crates/batchalign/src/fa/utr.rs` | `run_utr_pass()`, `inject_utr_timing()`, partial-window logic |
| FA library | `crates/batchalign/src/fa/` | Grouping, extraction, DP alignment, injection, postprocessing |
| Worker IPC | `python/batchalign/inference/fa.py` — `batch_infer_fa()` | Loads Whisper/Wave2Vec, returns token timestamps |

---

## `@Options: NoAlign` — strict pass-through

Files containing `@Options: NoAlign` are **returned completely unchanged**.
The pipeline performs zero modifications: no timestamps are added, removed,
or adjusted, no `%wor` tier is generated or updated, and no decision tiers
(`%xalign`, `%xrev`) are written.

The rationale is that a researcher who sets `@Options: NoAlign` has explicitly
opted this file out of all alignment processing.  Batchalign must respect that
decision unconditionally — including for cleanup passes that might seem benign
(such as monotonicity enforcement).  Any existing timestamps, even backward
ones from a previous run, are the researcher's responsibility.

If a file with `@Options: NoAlign` carries validation errors from a previous
FA run, the correct fix is to repair the file manually or remove the option,
re-run align, and re-add the option if still needed.

Implementation: `run_fa_from_ast` checks `is_no_align(&chat_file)` immediately
after parsing (before media resolution, pre-validation, and all FA logic) and
returns `Ok(FaResult { chat_text: to_chat_string(&chat_file), ..empty })`.

---

## Pre-validation gate

`align` requires CHAT Level 2 (parseable + headers + valid main tiers) before
running inference. Invalid files are rejected immediately with a typed error
rather than consuming GPU time. See
[Command Contracts](../../architecture/command-contracts.md) for the validity
level definitions.

Implemented in `crates/batchalign/src/commands/align.rs`:
```rust,ignore
validate_to_level(chat, ValidationLevel::MainTiers)?;
```

---

## Cache key structure

FA results are keyed by BLAKE3 hash over:
- word sequence (normalized)
- audio segment (file path + byte range)
- FA engine name + version
- language code

UTR ASR results are cached separately per audio segment (file path + start_ms
+ end_ms). Segment cache hits avoid re-running ASR on already-processed
windows during the partial-window optimization.

Cache implementation: `crates/batchalign/src/cache/` (hot: moka,
cold: SQLite). Bypass with global `--override-media-cache`.

---

## 3-tier reuse strategy

Each FA group is checked for reusability in priority order before inference:

**Tier 1: Reuse from `%wor` tier**  
If all utterances in a group have clean `%wor` timing from a previous run,
those word timings are used directly without re-processing. This is the fastest
path and requires no worker inference.

**Tier 2: Cache hit**  
If Tier 1 doesn't apply, check the shared result cache by the group's cache key.
A cache hit means the exact same audio + word sequence was previously processed
on this engine version — reuse the timings without worker dispatch.

**Tier 3: Cache miss**  
Send the group to the FA worker for inference. After the worker returns timings,
they are written back to the cache so subsequent jobs hitting the same content
will use Tier 2.

This three-level hierarchy is logged during FA execution as `"FA partition
(reused from %wor / cache hits / misses)"` so operators can track reuse efficiency.

Implementation: `crates/batchalign/src/fa/mod.rs:345–391`.

---

## Worker IPC: FA task (V2 protocol)

```text
Client → Worker: execute_v2 request
{
  "task": "fa",
  "prepared_audio": { path, start_ms, end_ms, sample_rate },
  "prepared_text":  { words: [...], language: "eng" },
  "engine": "wav2vec" | "whisper"
}

Worker → Client: execute_v2 response
{
  "tokens": [
    { "word": "hello", "start_s": 0.12, "end_s": 0.45 },
    ...
  ]
}
```

The Rust server converts seconds → milliseconds and runs Hirschberg DP
alignment (`dp_align.rs`) to map FA tokens back to CHAT transcript words.

---

## UTR strategy resolution

`resolve_strategy()` in `crates/batchalign/src/runner/dispatch/utr.rs:80–114`:

**Auto strategy (default):** Always returns `GlobalUtr` regardless of language or overlap markers.

The previous auto-detection logic (which selected `TwoPassOverlapUtr` for English
files with `+<` or CA overlap markers) was **disabled 2026-03-30** due to:
1. Operator-reported alignment regressions on real files
2. End-time overlap bug in `enforce_monotonicity()` — it only corrects start-time
   violations, not end-time overlaps, so overlapping utterance bullets go uncorrected
3. Two-pass algorithm was only tuned on 4 corpora, not broadly validated

**Explicit overrides:**
- `--utr-strategy global` → `GlobalUtr` (single-pass monotonic recovery)
- `--utr-strategy two-pass` → `TwoPassOverlapUtr` (experimental; overlap-aware,
  gated until the end-time overlap bug is resolved)

When both `total_audio_ms` and `max_group_ms` are available, a `GroupingContext` is
passed to `TwoPassOverlapUtr` so it can detect and avoid the wider-window regression
on non-English files. This is only consulted on explicit `--utr-strategy two-pass`;
`Auto` does not reach this code path.

---

## Incremental processing (`--before`)

When `--before PATH` is provided, `process_fa_incremental()` in
`fa_pipeline.rs` diffs the old and new CHAT files, classifies each utterance
as Added/Removed/Modified/Unchanged, and only runs FA on content that changed.
Stable `%wor` entries from the old file are copied directly, skipping the FA
worker entirely for unchanged groups.

See [Incremental Processing](../../architecture/incremental-processing.md).

---

## FA grouping constraints

`group_utterances()` enforces two independent split constraints. A group is
flushed when either is exceeded by adding the next utterance:

- **Time window** — configured via `AlignOptions.max_group_ms` (default 20 000 ms)
- **Character-token limit** — `WHISPER_FA_MAX_LABEL_TOKENS = 448` (constant in
  `grouping.rs`). Whisper's CTC FA counts every character of every word as one
  label token. Exceeding 448 raises a hard Python `ValueError`. Dense languages
  (Spanish, any long-word corpus) can hit this inside a normal time window.

The flush guard is skipped only when the current group is empty — if one
utterance alone exceeds 448 chars it is sent as its own group (fail gracefully
rather than drop silently).

See [Forced Alignment: FA grouping strategy](../../reference/forced-alignment.md#fa-grouping-strategy)
for the full rationale, flowchart, and edge cases.

---

## Pre-grouping preparation steps

Before FA grouping, the AST undergoes two surgical modifications to prepare utterance
bullets for inference:

**Narrow bullet rescue** (enabled always)  
When `transcribe` writes a bullet that is too narrow to contain its words (e.g., 22
words in 380 ms = 58 wps, physically impossible), the rescue pre-pass detects and
expands that bullet into the trailing inter-utterance gap. This gives FA a wide-enough
audio window to find the actual speech. After FA finishes, `update_utterance_bullet`
overwrites the rescued range with the FA word span (tighter), so the rescue is
self-healing and auditable.

Implementation: `crates/batchalign/src/fa/mod.rs:247–267`. Decisions (which utterances
were rescued) are recorded and later injected as `%xalign` tiers for audit trail.

**Edge filler expansion** (enabled always)  
UTR-assigned bullets may be too narrow to include trailing or leading fillers whose
audio lives in inter-utterance gaps. This step expands utterance bullets to cover
those edge fillers, ensuring they are included in the FA group.

Implementation: `crates/batchalign/src/fa/mod.rs:269–272`.

## Compound filler splitting

CHAT underscore-joined fillers (`&-you_know`, `&-sort_of`) are split at
underscores before being sent to the FA engine because ASR models return them
as separate words. After alignment, the N timings are merged back into one span.
Only `WordCategory::Filler` words are split — regular compounds (`ice_cream`)
are unchanged.

See `crates/batchalign/src/fa/COMPOUND_FILLER_ALIGNMENT.md`.

---

## Decision tier injection

The align pipeline records all structural decisions (modifications to utterance
bullets, %wor generation, monotonicity repairs, etc.) in `%xalign` and `%xrev`
tiers for complete auditability.

**Decision sources** (in order):
1. **Narrow bullet rescue** — utterances whose bullets were pre-expanded before
   grouping (see "Pre-grouping preparation steps")
2. **FA word timing injection** — word boundaries, timing drops, speech gaps
3. **Experimental bullet repair** — only if `--bullet-repair` flag is enabled
4. **Monotonicity enforcement** — start-time regressions stripped, end-time overlaps clamped

All previous `%xalign`/`%xrev` tiers are stripped before injection (even on clean
re-runs with no new decisions) to prevent stale decision duplication across reruns.

Implementation: `crates/batchalign/src/fa/mod.rs:506–537`. The injection layer is in
`crates/talkbank-transform/src/decisions/`.

---

## Post-FA validation

After FA finishes, the CHAT file is validated at Level 2 (output gate equivalent to
[Command Contracts: align post-validation](../../architecture/command-contracts.md#align-post-validation)).
Validation errors are **warnings only** — cross-speaker overlap is normal in
conversation data and non-fatal. If critical errors appear (e.g., invalid tier
codes), they are logged but do not fail the job.

Implementation: `crates/batchalign/src/fa/mod.rs:539–554`.

---

## Testing

```bash
# Fast unit tests (no ML models)
bazel test //...

# FA-specific tests with real models (only on net, 256 GB RAM)
cargo nextest run --profile ml -E 'test(fa::)'

# Incremental processing tests
cargo nextest run -p batchalign-cli --test incremental
```

Key test locations:
- `crates/batchalign/src/fa/` — unit tests for grouping, injection, UTR
- `crates/batchalign/tests/` — integration tests for the FA pipeline

---

## Related developer documentation

- [Command Flowcharts: align](../../architecture/command-flowcharts.md#align) — detailed runtime flowchart with 3 diagrams
- [Forced Alignment](../../reference/forced-alignment.md) — algorithm design, prerequisites
- [Dynamic Programming](../../../architecture/parser-and-grammar/dynamic-programming.md) — Hirschberg aligner
- [Incremental Processing](../../architecture/incremental-processing.md) — `--before` mechanics
- [Overlap Encoding](../../architecture/overlap-encoding.md) — `+<` and CA marker handling
- [Command Contracts](../../architecture/command-contracts.md) — pre/post validation gates
- [Adding Commands](../adding-commands.md) — use `align` as the reference implementation for `PerFileTransform`
