# E375: Scoped annotation parse error

## Description

Scoped annotation parse error

## Metadata

- **Error Code**: E375
- **Category**: Parser bugs (experimental)
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E3xx_main_tier_errors/E350_unexpected_annotation_node.cha`
**Trigger**: Try to trigger internal parser bug in annotation parsing
**Expected Error Codes**: E375

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	hello [[[[ test ]]]] world .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
