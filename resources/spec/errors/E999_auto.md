# E999: Unknown error

## Description

Unknown error

## Metadata

- **Error Code**: E999
- **Category**: Alignment count mismatch
- **Level**: file
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/terminator_alignment.cha`
**Trigger**: Main tier terminator should align with %mor terminator
**Expected Error Codes**: E707

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Terminators (. ! ?) align for %mor and %gra
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie
@Comment:	ERROR: %mor missing terminator (should end with .)
@Comment:	Main tier: 3 words + terminator = 4 alignable
@Comment:	Mor tier: Only 3 items (missing terminator)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
