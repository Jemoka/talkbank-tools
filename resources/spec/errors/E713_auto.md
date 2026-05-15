# E713: Gra head index invalid

## Description

A `%gra` relation has a head index that falls outside the valid range
`0..=N`, where `N` is the number of `%mor` chunks in the utterance. Index
`0` is reserved for the ROOT head; otherwise the head index must point to
an existing chunk.

## Metadata

- **Error Code**: E713
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Example 1

**Trigger**: head index exceeds %mor chunk count (per-relation validation,
cardinalities match)
**Expected Error Codes**: E713

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: head index 9 is out of range for 3 mor chunks
*CHI:	I want .
%mor:	pro|I v|want .
%gra:	1|2|SUBJ 2|0|ROOT 3|9|PUNCT
@End
```

## Expected Behavior

With `%mor` providing 3 chunks and `%gra` providing 3 relations (matching
counts), the third relation's head index `9` is invalid. The parser should
successfully parse this CHAT file, but validation should report E713 on
that relation.

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra). Each
%gra relation is `index|head|label`; `head` must be `0` for ROOT or the
1-based position of an existing %mor chunk.

## Notes

- E713 applies to per-relation head-index validation.
- Count-cardinality mismatches between %mor and %gra are reported as E720.
