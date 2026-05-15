# E718: Sin count mismatch - too few sin tokens

## Description

Sin count mismatch - too few sin tokens

## Metadata

- **Error Code**: E718
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/E718_sin_count_too_few.cha`
**Trigger**: Main tier has 3 words, but %sin has only 2 tokens
**Expected Error Codes**: E718

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%sin:	POINT REACH
@Comment:	ERROR: Main tier has 3 words but %sin only has 2 tokens (missing gesture for cookie)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
