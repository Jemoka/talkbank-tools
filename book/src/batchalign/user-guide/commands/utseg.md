# utseg

**Status:** Current
**Last updated:** 2026-05-05 06:53 EDT

Re-segment utterance boundaries in an existing CHAT transcript. Text-only
— no audio involved. The model selected per language is either a trained
BERT per-word boundary classifier (eng / cmn,zho / yue) or, for other
languages, Stanza constituency parsing where it is available.

`transcribe` already runs this same step at the end of every run
(`with_utseg = true` is the default in the transcribe pipeline). The
standalone `utseg` command is for already-existing corpora — files
transcribed elsewhere, hand-typed transcripts, or older BA2 output —
where utterances run on into long blobs and need to be split.

---

## Quick start

```bash
# Re-segment a single file in place
batchalign3 utseg file.cha --lang eng

# Re-segment a corpus directory
batchalign3 utseg corpus/ -o segmented/ --lang eng

# Use the remote server
batchalign3 --server http://your-server:8001 utseg corpus/ -o out/ --lang eng
```

---

## Pipeline

Each file is dispatched on its own — `dispatch_utseg_job` in
`crates/batchalign/src/execution/utseg.rs` calls
`gateway.utseg_batch(&[one_file], lang)` per file and writes that
file's result to disk before starting the next. (This replaced an
earlier "pool everything across all files, batch through one worker,
write at end" pattern, which lost the entire run's work on a daemon
redeploy mid-batch. The per-file shape limits a mid-run interruption
to losing only files currently in flight.) Per-file concurrency is
bounded by `plan.kernel_plan.file_parallelism_hint` (clamped to ≥ 1),
the same heuristic as `fa_pipeline.rs`.

```mermaid
flowchart TD
    start([utseg invoked]) --> parse[Parse one file → AST]
    parse --> collect[collect_payloads\nExtract word sequences per utterance]
    collect --> worker[gateway.utseg_batch(&[file], lang)\n→ BERT assignments\nor Stanza constituency trees]
    worker --> apply[Apply segmentation\nSplit/merge utterances at predicted boundaries]
    apply --> merge_check{--merge-abbrev?}
    merge_check -->|Yes| merge[merge_abbreviations]
    merge_check -->|No| serialize
    merge --> serialize[Serialize → .cha output]
    serialize --> done([Write file's .cha; next file in pool])
```

---

## Options

### Path options

| Option | Meaning |
| --- | --- |
| `PATHS...` | Input `.cha` files or directories |
| `-o`, `--output DIR` | Output directory (omit to overwrite in place) |
| `--file-list FILE` | Read input paths from a text file |
| `--in-place` | Explicit in-place flag |

In-place rewrites with `--file-list` on a large corpus do appear
file-by-file as the run progresses (each file is written to disk
before the next file's worker call starts). This is a deliberate
property of the per-file dispatch shape — interruption mid-run loses
only the files currently in flight, not the entire batch. Splitting
the file list into smaller chunks is therefore unnecessary for
incremental visibility, though it remains useful for managing memory
or scheduling.

### utseg options

| Option | Default | Meaning |
| --- | --- | --- |
| `--lang CODE` | `eng` | 3-letter ISO language code |
| `-n`, `--num-speakers N` | `2` | Number of speakers |
| `--merge-abbrev` | off | Merge abbreviations in the output |

---

## What changes in the `.cha` file

- Utterance boundaries (`*SPK:` lines) are recomputed — utterances may be
  split or merged
- Existing `%mor` and `%gra` tiers on recomputed utterances will be
  invalidated; re-run `morphotag` after `utseg` if those tiers are needed
- No audio is involved

---

## Language support

Per-language model selection is driven by `_RESOLVER["utterance"]` in
`python/batchalign/models/resolve.py`:

| `--lang` | Model loaded | Source |
|----------|--------------|--------|
| `eng` | `talkbank/CHATUtterance-en` (BERT per-word classifier) | TalkBank fine-tune |
| `cmn` / `zho` (Mandarin) | `talkbank/CHATUtterance-zh_CN` (BERT) | TalkBank fine-tune |
| `yue` (Cantonese) | `PolyU-AngelChanLab/Cantonese-Utterance-Segmentation` (BERT) | PolyU AngelChanLab |
| any other language | Stanza constituency parser, where available | Stanza |

The English BERT is **not** applied cross-lingually — running `utseg
--lang fra` does not load `CHATUtterance-en`. Languages with no entry in
the resolver fall through to the Stanza constituency path. Stanza ships
constituency models for ~11 languages (en, de, es, it, pt, da, id, ja,
tr, vi, zh-hans); for any other language, `utseg` currently produces
no splits and the file passes through unchanged.

See [Utterance Segmentation](../../reference/utterance-segmentation.md)
for the algorithm details and the
[Stanza Capability Registry](../../architecture/stanza-capability-registry.md)
for the per-language processor availability table.

---

## Related documentation

- [Utterance Segmentation](../../reference/utterance-segmentation.md) — algorithm and model details
- [Stanza Capability Registry](../../architecture/stanza-capability-registry.md) — which languages support constituency parsing
- [Command I/O: utseg](../../reference/command-io.md#5-utseg) — I/O patterns and mutation behavior
- [Command Flowcharts: utseg](../../architecture/command-flowcharts.md#utseg) — full architecture flowchart
