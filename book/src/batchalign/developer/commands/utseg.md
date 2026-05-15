# utseg — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 08:18 EDT

Implementation guide for the `utseg` command. For user-facing documentation,
see [User Guide: utseg](../../user-guide/commands/utseg.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `UtsegArgs` | lang, num-speakers |
| Command definition | `crates/batchalign/src/commands/utseg.rs` | `CommandDefinition` impl |
| Utseg orchestration | `crates/batchalign/src/utseg.rs` | Cross-file batching, cache, boundary application |
| Batch dispatch | `crates/batchalign/src/runner/dispatch/infer_batched.rs` | Shared with morphotag and translate |
| Worker IPC | `python/batchalign/inference/utseg.py` — `batch_infer_utseg()` | Loads Stanza constituency, returns raw parse trees |
| Boundary application | `crates/batchalign/src/utseg.rs` | Maps predicted boundaries back to CHAT utterance structure |

Local submissions (auto-daemon or loopback `--server`) use `paths_mode=true`
as of 2026-04-14: the CLI posts source/output path lists instead of CHAT
bytes. See [Submission Modes](../../reference/command-io.md#submission-modes-paths_modetrue-vs-paths_modefalse).

---

## Caching behavior

Text NLP tasks (`utseg`, `translate`, `morphotag`) do not use the utterance cache.
Boundaries are computed deterministically from word sequence and language during
each inference run — no per-utterance caching occurs. (Audio tasks like forced
alignment and UTR ASR do cache, because those operations are expensive per-file.)

---

## Worker IPC: utseg task

```text
batch_infer request:
{
  "task": "utseg",
  "items": [
    { "words": ["hello", "world", "how", "are", "you"], "lang": "eng" },
    ...
  ]
}

batch_infer response:
[
  { "boundaries": [2, 5], "parse_tree": "..." },
  ...
]
```

`boundaries` is a list of word indices where utterance breaks are inserted.
The Rust `utseg.rs` library maps these back to CHAT utterance splits/merges.

---

## Stanza constituency availability

About 11 languages have Stanza constituency models. Languages without
constituency support fall back to punctuation-based boundary detection. The
available processors are queried at worker startup via
`python/batchalign/worker/_stanza_capabilities.py` — never hardcoded.

---

## Pre-validation gate

`utseg` requires CHAT Level 1 (parseable + valid headers). Gate in
`crates/batchalign/src/utseg.rs`. Implemented via
`validate_to_level(chat, ValidationLevel::StructurallyComplete)`.

---

## Testing

```bash
bazel test //...
cargo nextest run -p batchalign-cli -E 'test(utseg::)'
# ML golden tests — only on net
cargo nextest run --profile ml -E 'test(utseg::golden)'
```

---

## Related developer documentation

- [Command Flowcharts: utseg](../../architecture/command-flowcharts.md#utseg)
- [Utterance Segmentation](../../reference/utterance-segmentation.md)
- [Stanza Capability Registry](../../architecture/stanza-capability-registry.md)
