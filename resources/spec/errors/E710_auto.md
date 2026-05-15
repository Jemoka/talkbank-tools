# E710: Invalid GRA format

## Description

Invalid GRA format

## Metadata
- **Status**: implemented

- **Error Code**: E710
- **Category**: Dependent tier parsing
- **Level**: tier
- **Layer**: parser

## Example 1

**Source**: `E7xx_tier_parsing/E708_invalid_gra_format.cha`
**Trigger**: %gra relation without enough pipe separators
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%gra:	1-2-SUBJ 2|0|ROOT
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
