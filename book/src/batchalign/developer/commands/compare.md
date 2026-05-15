# compare — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 08:18 EDT

Implementation guide for the `compare` command. For user-facing
documentation, see [User Guide: compare](../../user-guide/commands/compare.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `CompareArgs` | lang, num-speakers |
| Command definition | `crates/batchalign/src/commands/compare.rs` | `CommandDefinition` impl, gold-file discovery |
| Compare library | `crates/batchalign/src/compare.rs` | `compare()` — produces `ComparisonBundle` |
| Released materializer | `crates/batchalign/src/compare.rs` — `materialize_released()` | Projects %mor/%gra/%wor from main to gold, injects `%xsrep`/`%xsmor` |
| Benchmark materializer | `crates/batchalign/src/compare.rs` — `materialize_main_annotated()` | Annotates main transcript with `%xsrep`/`%xsmor` (internal to benchmark) |
| CSV writer | `crates/talkbank-transform/src/compare/metrics.rs` — `format_metrics_csv()` | Typed metrics model → CSV output |

Local submissions (auto-daemon or loopback `--server`) use `paths_mode=true`
as of 2026-04-14: the CLI posts source/output path lists instead of CHAT
bytes. Compare derives `FILE.gold.cha` first and falls back to
`template.gold.cha` at execution time inside the same directory.

---

## ComparisonBundle

The central typed model. Produced by `compare()` in `talkbank_transform::compare`:

```rust,ignore
pub struct ComparisonBundle {
    pub main_utterances: Vec<UtteranceComparison>,  // per-utterance main-side comparisons
    pub gold_utterances: Vec<UtteranceComparison>,  // per-utterance gold-side comparisons
    pub gold_word_matches: Vec<GoldWordMatch>,      // structural word matches (gold → main)
    pub metrics: CompareMetrics,                     // aggregate WER + per-POS breakdown
}
```

Each `UtteranceComparison` contains:
- `utterance_index` — position in file
- `speaker` — speaker code
- `tokens` — comparison tokens (status: Match/ExtraMain/ExtraGold, with optional POS)

Each `GoldWordMatch` maps one word position in a gold utterance to one position in a main
utterance, establishing the structural alignment used by projection.

Two materializer functions consume this bundle:
- `materialize_released()` → gold-projected output (for released compare command)
- `materialize_main_annotated()` → main-annotated output (internal path, used by benchmark)

---

## Gold projection semantics

The gold projection process (`project_gold_structurally()`) iterates each gold utterance
and determines whether to copy or reconstruct comparison tiers.

For each gold utterance, a three-step check (`exact_projection_source()`) determines if
all gold words have **perfect structural alignment** to a single main utterance:

1. **Match completeness:** Every gold word position must have exactly one match
2. **Uniqueness:** Matches must map to distinct gold positions
3. **Mono-utterance:** All matches must originate from a single main utterance
4. **Word parity:** Compared word counts must match between gold and the source main utterance
5. **Alignable parity:** Alignable word counts (same universe) must match
6. **No errors:** All tokens must have `Match` status (no insertions/deletions)

**If exact match found (all 6 conditions pass):**
Copy `%mor`, `%gra`, `%wor` tiers directly from the source main utterance to the gold
utterance. This is the safest projection path.

**Otherwise (any condition fails):**
Reconstruct a projected `%mor` tier from the partial matches in `gold_word_matches`.
This handles cases where words are reordered, inserted, or deleted. The projected
`%mor` is built directly from the bundle's typed data, never from serialized text.

---

## CSV output model

```rust,ignore
pub struct CompareMetricsRow {
    pub label:       MetricLabel,    // "aggregate" or POS string
    pub wer:         f64,
    pub accuracy:    f64,
    pub matches:     u32,
    pub insertions:  u32,
    pub deletions:   u32,
    pub total_words: u32,
}
```

Written once at the serialization boundary via `csv::Writer`. No ad-hoc string
assembly.

---

## Testing

```bash
# Unit tests (no ML models)
bazel test //...
cargo nextest run -p batchalign-cli -E 'test(compare::)'

# Golden tests (real Stanza for morphotag step — only on net)
cargo nextest run --profile ml -E 'test(compare::golden)'
```

---

## Related developer documentation

- [Command Flowcharts: compare](../../architecture/command-flowcharts.md#compare)
- [BA2 Compare Migration](../../migration/ba2-compare-migration.md) — how compare was re-architected from BA2
- [Adding Commands](../adding-commands.md) — use `compare` as the reference for `ReferenceProjection`
- [benchmark developer reference](benchmark.md) — composite command that calls compare internally
