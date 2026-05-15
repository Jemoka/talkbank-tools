# E720: Mor-Gra count mismatch

## Description

The number of `%mor` chunks does not equal the number of `%gra` relations
for an utterance. `%gra` aligns 1-to-1 with `%mor` chunks (not items — a
`%mor` item with post-clitics produces multiple chunks).

## Metadata

- **Error Code**: E720
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: %gra aligns to %mor chunks, not items!
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie .
%gra:	1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|4|EXTRA 5|2|PUNCT
@Comment:	ERROR: %mor has 4 chunks (3 words + terminator) but %gra has 5 relations
@End
```

## Expected Behavior

Validation reports E720 when the number of `%mor` chunks and `%gra`
relations disagree. The diagnostic includes a column-by-column layout
showing both tiers so the author can see which entries are missing or
extra.

**Trigger**: %mor has 4 chunks but %gra has 5 relations.

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra). Each
`%mor` chunk requires a corresponding `%gra` relation; clitics in `%mor`
produce additional chunks that also need relations.

## Notes

- E720 is emitted by the `align_mor_to_gra` alignment pass in
  `talkbank-model`.
- E712 (GraInvalidWordIndex) and E713 (GraInvalidHeadIndex) are reserved
  for per-relation index validation; count mismatches use E720.
