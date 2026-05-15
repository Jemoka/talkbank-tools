# E719: Sin count mismatch - too many sin tokens

## Description

Sin count mismatch - too many sin tokens

## Metadata

- **Error Code**: E719
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/E719_sin_count_too_many.cha`
**Trigger**: Main tier has 2 words, but %sin has 3 tokens
**Expected Error Codes**: E719

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	want cookie .
%sin:	POINT REACH GRAB
@Comment:	ERROR: Main tier has 2 words but %sin has 3 tokens (extra GRAB)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
