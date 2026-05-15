# REPEAT -- Mark Utterances Containing Revisions

**Status:** Current
**Last updated:** 2026-05-12 11:31 EDT

## Purpose

Reimplements CLAN's `repeat` command, which adds a `[+ rep]` postcode to utterances from a target speaker that contain revision markers. Only utterances that do not already have `[+ rep]` are modified.

## Usage

```bash
chatter clan repeat --speaker CHI file.cha
```

## Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--speaker` | speaker code | *(required)* | Target speaker to process. Only utterances from this speaker are checked. |

## Revision Markers Detected

Per `is_revision_kind()` at
`crates/clan-core/src/transforms/repeat.rs:81-87`, three of the
four `RetraceKind` variants trigger `[+ rep]`:

- `[//]` — full retrace (`RetraceKind::Full`)
- `[///]` — multiple retracing (`RetraceKind::Multiple`)
- `[/-]` — reformulation (`RetraceKind::Reformulation`)

Note: Simple partial repetition (`[/]`, `RetraceKind::Partial`)
does **not** trigger the `[+ rep]` marker — that's the
"non-revision" case. (Earlier versions of this doc listed a `[/?]`
"uncertain retracing" marker as the fourth case; there is no such
marker — the grammar has exactly four retrace tokens, all listed
above and at `book/src/chat-format/retraces.md`.)

## Behavior

For each utterance from the target speaker, the transform checks whether the main-tier content contains any revision markers. If revision markers are found and the utterance does not already have a `[+ rep]` postcode, one is appended.

Utterances from other speakers are left unchanged.

## Differences from CLAN

- Operates on AST rather than raw text.
- Uses the framework transform pipeline (parse -> transform -> serialize -> write).
