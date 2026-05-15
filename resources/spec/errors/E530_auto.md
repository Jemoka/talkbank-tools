# E530: Lazy gem inside background

## Description

Lazy gem inside background

## Metadata

- **Error Code**: E530
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/lazy-gem-inside-bg.cha`
**Trigger**: See example below
**Expected Error Codes**: E530

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Bg:activity
@Comment:	We are inside a @Bg/@Eg scope
@G:	playing with blocks
@Comment:	ERROR: @G (lazy gem) should not be allowed inside @Bg/@Eg scope
@Eg:activity
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
