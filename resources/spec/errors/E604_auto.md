# E604: Empty GRA relation

## Description

Empty GRA relation

## Metadata

- **Error Code**: E604
- **Category**: Dependent tier parsing
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E7xx_tier_parsing/E707_empty_gra_relation.cha`
**Trigger**: %gra tier with empty relation (consecutive spaces)
**Expected Error Codes**: E604

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%gra:	1|2|SUBJ  2|0|ROOT
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
