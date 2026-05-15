# E359: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E359
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E359_unmatched_longfeature_end.cha`
**Trigger**: See example below
**Expected Error Codes**: E359

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	happy birthday to you &}l=singing .
*CHI:	another utterance .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
