# E241: Illegal Untranscribed Marker 'xx'

## Description

The marker 'xx' is used for untranscribed speech, but this is not allowed in CHAT. The correct marker for untranscribed speech is 'xxx' (three x's).

## Metadata

- **Category**: word_validation
- **Level**: word
- **Layer**: validation

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said xx today .
@End
```

## Expected Behavior

- **Parser**: Should succeed - 'xx' is syntactically valid as a word
- **Validator**: Should report E241 - 'xx' is not a valid untranscribed marker

## CHAT Rule

Untranscribed speech must be marked with 'xxx' (three x's). Single 'x' represents a single unintelligible phoneme, 'xx' is not a valid CHAT marker. Use 'xxx' for untranscribed speech segments.

## Notes

This is a semantic validation error that occurs at the word level. The parser accepts 'xx' as a valid word syntactically, but the validator must check that it follows CHAT conventions. Two-letter 'xx' combinations are sometimes mistakenly used instead of the correct 'xxx' marker.
