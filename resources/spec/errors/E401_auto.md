# E401: Duplicate dependent tiers

## Description

Duplicate dependent tiers

## Metadata

- **Error Code**: E401
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/duplicate-dependent-tier.cha`
**Trigger**: See example below
**Expected Error Codes**: E401

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie .
%mor:	pro|I v|want n|cookie .
@Comment:	ERROR: Duplicate %mor tier for the same utterance
*CHI:	you have ball .
%gra:	1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
%gra:	1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT
@Comment:	ERROR: Duplicate %gra tier for the same utterance
@End
```

## Example 2

**Source**: `E4xx_alignment_errors/E401_duplicate_mor.cha`
**Trigger**: Multiple %mor tiers for the same utterance
**Expected Error Codes**: E401

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie .
%mor:	pro|I v|want n|cookie .
@Comment:	ERROR: Duplicate %mor tier should trigger E401
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
