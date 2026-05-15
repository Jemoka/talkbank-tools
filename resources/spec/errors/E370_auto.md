# E370 — Structural order error

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

A structural ordering violation in the utterance content, such as groups or
replacements that do not align correctly with dependent tier items.

## Metadata

- **Error Code**: E370
- **Category**: Alignment count mismatch
- **Level**: utterance
- **Layer**: validation
- **Status**: not_implemented

## Example 1

**Source**: `E4xx_alignment_errors/multiple_replacements.cha`
**Trigger**: Multiple replacement forms with word count mismatch
**Expected Error Codes**: E316, E600

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I <wanna> [: want to] <the> [: a] cookie .
%mor:	pro|I v|want n|cookie .
@End
```

**Note:** This example currently produces E316 (parse error on the group
syntax) and E600 (malformed dependent tier) instead of E370. The E370
validation check is not yet implemented.

## Example 2

**Trigger**: Groups with correction alignment
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I <want the> [: need the] cookie .
%mor:	pro|I v|need n|cookie .
@End
```

## Example 3

**Trigger**: Nested groups with corrections
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I <want the big> [: need a large] cookie .
%mor:	pro|I v|need n|cookie .
@End
```

## Notes

- E370 is not yet implemented as a validation check.
- The examples demonstrate the scenarios that should trigger E370 once
  implemented.
