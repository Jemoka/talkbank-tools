# E347 — Unbalanced cross-speaker overlap (indexed markers)

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

An **indexed** top overlap region (e.g., `⌈2...⌉2`) on one speaker has no
matching indexed bottom overlap region (`⌊2...⌋2`) from a different speaker,
or vice versa. Reported as a warning because some onset-only marking
conventions exist.

Unindexed markers are deliberately not checked — multi-party overlaps without
numeric indices are inherently ambiguous (validated empirically against SBCSAE,
CLAPI, and Forrester corpora).

## Metadata

- **Error Code**: E347
- **Category**: validation
- **Level**: cross_utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: Indexed top overlap `⌈2...⌉2` with no matching bottom `⌊2...⌋2`
**Expected Error Codes**: E347

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	⌈2hello⌉2 .
*MOT:	hi .
@End
```

## Expected Behavior

Validation should report E347 (warning) on the `*CHI:` utterance because the
indexed top overlap `⌈2...⌉2` has no matching bottom overlap `⌊2...⌋2` from
MOT or any other speaker. The fix is either to add a matching `⌊2...⌋2` on
MOT's utterance or remove the overlap markers.

## CHAT Rule

<https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

Overlap markers come in paired sets: top markers (`⌈⌉`) on one speaker's
utterance must have corresponding bottom markers (`⌊⌋`) on the overlapping
speaker's utterance. When indexed (2-9), the indices must match.

## Notes

- Only indexed markers (2-9) are checked. Unindexed markers are skipped.
- Supports 1:N matching — one top can match multiple bottoms from different
  speakers.
- Implementation: `validation/cross_utterance/mod.rs:229-409`
