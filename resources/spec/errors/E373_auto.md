# E373: InvalidOverlapIndex

## Description

An overlap marker has an index value outside the valid range. For CA
overlap brackets (`⌈⌉⌊⌋`), the index must be 2–9. For scoped overlap
annotations (`[<]`, `[>]`), the index must be 1–9.

## Metadata
- **Status**: implemented

- **Error Code**: E373
- **Category**: overlap
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E373_invalid_overlap_index.cha`
**Trigger**: CA overlap bracket with index 1 (valid range is 2-9)
**Expected Error Codes**: E373

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Overlap index must be 2-9
*CHI:	⌈1 hello ⌉ .
@End
```

## Example 2

**Trigger**: Scoped overlap annotation with index 0 (tree-sitter unreachable — produces E316)
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	MOT Mother, CHI Target_Child
@ID:	eng|test|MOT||female|||Mother|||
@ID:	eng|test|CHI||male|||Target_Child|||
*MOT:	I think [<0] so .
*CHI:	yeah [>0] .
@End
```

## Expected Behavior

Validation should report E373. Overlap indices must be within valid ranges:
1–9 for scoped overlap annotations, 2–9 for CA overlap point brackets.

## CHAT Rule

Overlap markers use numeric indices to pair overlapping speech segments
between speakers. Valid indices are single digits (1–9 for scoped, 2–9
for CA brackets). See the CHAT manual on overlap conventions.
