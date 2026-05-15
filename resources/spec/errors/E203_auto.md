# E203: Invalid form type marker

## Description

Word contains an invalid or undeclared `@` form type marker (e.g., `dog@b@c` has multiple stacked markers).

## Metadata
- **Status**: implemented

- **Error Code**: E203
- **Category**: validation
- **Level**: word
- **Layer**: parser

## Example 1

**Source**: `E2xx_word_errors/E203_invalid_form_marker.cha`
**Trigger**: See example below
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	dog@b@c .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
