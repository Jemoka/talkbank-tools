# E244: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E244
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E244_consecutive_stress.cha`
**Trigger**: See example below
**Expected Error Codes**: E244

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Stress markers cannot be consecutive
@Comment:	Invalid: 'ˈˈhello' - Two stress marks in a row
*CHI:	ˈˈhello .
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
