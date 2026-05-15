# translate — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 08:18 EDT

Implementation guide for the `translate` command. For user-facing
documentation, see [User Guide: translate](../../user-guide/commands/translate.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `TranslateArgs` | lang override |
| Command definition | `crates/batchalign/src/commands/translate.rs` | `CommandDefinition` impl |
| Translate orchestration | `crates/batchalign/src/translate.rs` | Cross-file batching, cache, `%xtra` injection |
| Batch dispatch | `crates/batchalign/src/runner/dispatch/infer_batched.rs` | Shared with morphotag and utseg |
| Injection | `crates/batchalign/src/translate.rs` | Writes `%xtra:` tiers from translation strings |
| Engine bootstrap | `python/batchalign/worker/_model_loading/translation.py::load_translation_engine()` | Resolves `_state.translate_backend` at worker startup: `GOOGLE` first, `SEAMLESS` (`facebook/hf-seamless-m4t-medium`) fallback if `googletrans` import fails |
| Worker IPC | `python/batchalign/inference/translate.py` — `batch_infer_translate()` | Iterates batch items, calls the resolved `translate_fn(text, src_lang)`, returns `raw_translation` per item. Sleeps 1.5s per item when backend is `GOOGLE` (rate limit). Pre-processing (Chinese space removal) happens in Rust before the call; post-processing in Rust after |

Local submissions (auto-daemon or loopback `--server`) use `paths_mode=true`
as of 2026-04-14: the CLI posts source/output path lists instead of CHAT
bytes. See [Submission Modes](../../reference/command-io.md#submission-modes-paths_modetrue-vs-paths_modefalse).

---

## Cache key structure

Translation cache keys (BLAKE3 hash of):
- Normalized utterance text
- Source language code
- Target language code (always `eng`)

---

## Worker IPC: translate task

```text
batch_infer request:
{
  "task": "translate",
  "items": [
    { "text": "Bonjour le monde.", "src_lang": "fra", "tgt_lang": "eng" },
    ...
  ]
}

batch_infer response:
[ "Hello world.", ... ]
```

---

## Pre-validation gate

`translate` requires CHAT Level 1.

## Idempotency

`inject_translation` (in `talkbank-transform::translate`) calls
`replace_or_add_tier`, which **overwrites** any existing `%xtra` tier on the
utterance. Re-running `translate` on a file that already has `%xtra` tiers
re-translates and replaces them. This diverges from BA2, which guarded
with `if i.translation: continue` and preserved the first translation.

## BA2 → BA3 migration notes

| Concern | BA2-jan9 | BA3 |
|---------|----------|-----|
| CLI shape | `batchalign translate IN_DIR OUT_DIR` (separate dirs) | `batchalign3 translate <dir-or-file>` (in-place by default) |
| Default engine | `googletrans` (dispatch.py: `"translate": "gtrans"`) | `googletrans`, with silent Seamless fallback if import fails |
| Concurrency | Sequential per utterance, with `time.sleep(1.5)` on Google | Batched cross-file dispatch, multiple worker groups per language, 1.5s sleep retained per-item on Google only |
| Re-run behavior | Skip already-translated utterances | Overwrite existing `%xtra` |
| Chinese (yue/zho) preprocessing | Inline in `gtrans.py` only; `seamless.py` did NOT strip spaces (BA2 bug) | Uniform `preprocess_for_translate` in Rust, applied before any backend |
| Per-item failure | Aborts the file | Logs warn, returns empty translation, batch continues; transient errors retry |
| Output tier | `%xtra` | `%xtra` (identical) |

**Tier-name clarification.** Neither BA2 nor BA3 produces a `%tra` tier.
Both versions emit `%xtra`. Any other translation-tier name observed in
the wild was not written by Batchalign.

---

## Testing

```bash
bazel test //...
cargo nextest run -p batchalign-cli -E 'test(translate::)'
```

---

## Related developer documentation

- [Command Flowcharts: translate](../../architecture/command-flowcharts.md#translate)
