# E348 — Unpaired overlap marker within utterance

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

Reserved for within-utterance overlap pairing violations: a closing marker
(`⌉` or `⌋`) without a preceding opening marker (`⌈` or `⌊`) in the same
utterance, or vice versa.

**Deliberately suppressed.** Within-utterance unpaired markers are almost always
legitimate cross-utterance overlap spans — the opening marker appears on one
utterance and the closing marker on a later utterance by the same speaker.
Cross-utterance overlap pairing is handled by E347, which correctly matches
indexed markers across speakers.

When E348 was temporarily enabled, it produced 2,152 false positives on
hand-edited SBCSAE data (commit `ff9e41e0`, 2026-03-19). The check was
suppressed after empirical validation.

Onset-only marking (opening without closing) is a legitimate Jeffersonian CA
convention — the transcriber marks where overlap begins, with the end implied.

## Metadata

- **Error Code**: E348
- **Category**: validation
- **Level**: utterance
- **Layer**: validation
- **Status**: not_implemented

## Example 1

**Trigger**: Opening overlap without closing in same utterance (legitimate
cross-utterance span, not an error)
**Expected Error Codes**: E348

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello ⌈ world .
*MOT:	yes .
@End
```

## Notes

- `check_overlap_pairing()` in `validation/utterance/overlap.rs` exists as a
  stub with empty match arms — this is a deliberate design decision.
- Cross-utterance overlap pairing is handled by E347 (indexed markers).
- E348 remains reserved for future use if a genuine within-utterance-only
  pairing violation is identified.
