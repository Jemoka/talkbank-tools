# E601: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E601
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E6xx_dependent_tier_errors/E601_invalid_dependent_tier.cha`
**Trigger**: See example below
**Expected Error Codes**: E326

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
badtier
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
