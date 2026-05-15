# E372: Nested quotation

## Description

Nested quotation

## Metadata

- **Error Code**: E372
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/nested-quotation.cha`
**Trigger**: See example below
**Expected Error Codes**: E372

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@ID:	eng|corpus|MOT|30;00.|female|||Mother|||
*MOT:	she said “I told him “go away” yesterday” .
@Comment:	ERROR: Nested quotations - "go away" is inside "I told him..."
@Comment:	Java detects this with state stack; Rust only counts balance
*CHI:	okay mommy .
*MOT:	he said “hello” and “goodbye” .
@Comment:	VALID: Two separate quotations, not nested
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
