# E528: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E528
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `E5xx_header_errors/E528_gem_label_mismatch.cha`
**Trigger**: See example below
**Expected Error Codes**: E528

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Bg:	Story
*CHI:	hello .
@Eg:	Game
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
