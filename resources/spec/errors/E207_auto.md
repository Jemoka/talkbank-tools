# E207: Unknown scoped annotation marker

## Description

Unknown scoped annotation marker

## Metadata

- **Error Code**: E207
- **Category**: Word validation
- **Level**: word
- **Layer**: parser

## Example 1

**Source**: `E2xx_word_errors/E207_multiple_form_types.cha`
**Trigger**: Scoped annotation with unrecognized marker
**Expected Error Codes**: E207

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [@ xyz] world .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
