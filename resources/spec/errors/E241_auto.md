# E241: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E241
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E241_illegal_untranscribed.cha`
**Trigger**: See example below
**Expected Error Codes**: E241

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Untranscribed markers must be xxx, yyy, or www
@Comment:	Invalid: 'xx' - Only xxx, yyy, www are allowed
*CHI:	xx .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
