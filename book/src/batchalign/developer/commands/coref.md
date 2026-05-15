# coref — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 08:18 EDT

Implementation guide for the `coref` command. For user-facing documentation,
see [User Guide: coref](../../user-guide/commands/coref.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `CorefArgs` | lang override |
| Command definition | `crates/batchalign/src/commands/coref.rs` | `CommandDefinition` impl |
| Coref orchestration | `crates/batchalign/src/coref.rs` | Full-document context assembly, worker dispatch, sparse injection |
| Injection | `crates/batchalign/src/coref.rs` | Writes sparse `%xcoref:` tiers |
| Worker IPC | `python/batchalign/inference/coref.py` — `batch_infer_coref()` | Loads Stanza coref model, returns chain structures |

Local submissions (auto-daemon or loopback `--server`) use `paths_mode=true`
as of 2026-04-14: the CLI posts source/output path lists instead of CHAT
bytes. See [Submission Modes](../../reference/command-io.md#submission-modes-paths_modetrue-vs-paths_modefalse).

---

## No caching

`coref` intentionally bypasses the utterance cache. Coreference chains span
the entire document — the same utterance has different coreference in different
document contexts, making per-utterance BLAKE3 keys meaningless. Every `coref`
invocation always calls the worker.

This is a deliberate architectural decision, not an oversight. Annotated in
the `coref.rs` orchestration module.

---

## Sparse output

`%xcoref:` tiers are only written on utterances that contain at least one
mention participating in a coreference chain. Most utterances in a file are
untouched. This makes `coref` output stable under incremental edits — adding
or removing utterances that don't participate in chains doesn't disturb the
existing annotations.

---

## Worker IPC: coref task

```text
batch_infer request:
{
  "task": "coref",
  "items": [
    { "sentences": [["hello", "world"], ["she", "said"]], "lang": "eng" }
  ]
}

batch_infer response:
[
  {
    "chains": [
      { "mentions": [[0, 0, 1], [1, 0, 1]], "representative": "hello world" }
    ]
  }
]
```

Each mention is `[sentence_idx, start_word, end_word]`.

---

## English-only restriction

Stanza's coreference model is English-only. The orchestration layer checks the
resolved language before dispatching; non-English files pass through with no
`%xcoref` tiers written and no error reported.

---

## Testing

```bash
bazel test //...
cargo nextest run -p batchalign-cli -E 'test(coref::)'
# Requires Stanza coref model
cargo nextest run --profile ml -E 'test(coref::golden)'
```

---

## Related developer documentation

- [Command Flowcharts: coref](../../architecture/command-flowcharts.md#coref)
