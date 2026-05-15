# E250: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E250
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E250_secondary_without_primary.cha`
**Trigger**: See example below
**Expected Error Codes**: E250

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Secondary stress requires primary stress
@Comment:	Invalid: 'ˌhello' - Secondary stress without primary
*CHI:	ˌhello .
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
