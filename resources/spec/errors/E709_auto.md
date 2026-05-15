# E709: Invalid grammar index

## Description

A `%gra` relation uses an invalid index. `%gra` indices are 1-indexed: the
first word is `1`, and `0` is reserved for the ROOT attachment in the
dependent slot (`n|0|ROOT`). Using `0` in the first (index) slot of a
relation triggers E709.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-13 22:00 EDT

- **Error Code**: E709
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E709_invalid_grammar_index.cha`
**Trigger**: First slot of a `%gra` relation is `0` (indices are 1-indexed)
**Expected Error Codes**: E709

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
%mor:	co|hello .
%gra:	0|0|ROOT 1|0|PUNCT
@End
```

## Expected Behavior

The `%gra` parser should report E709 when a relation's index (first slot)
is 0. Indices are 1-indexed; 0 is only valid in the dependent (second)
slot as the ROOT attachment target.

## CHAT Rule

See the CHAT manual on dependent tier formats (%gra). Each relation has
the form `<index>|<dependent>|<role>`. The index must be at least 1.

## Notes

- Non-numeric indices (e.g., `abc|0|ROOT`) are rejected at the grammar
  level and produce E600 instead; E709 fires only when the index is a
  valid integer but equals 0.
