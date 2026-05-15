# E539: Unsupported @Transcription Value

## Description

An `@Transcription` header contains a value that is not one of the recognized transcription types. The file parses successfully but the unsupported value is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E539
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	badtype
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E539 — unsupported @Transcription value 'badtype'

## CHAT Rule

The `@Transcription` header accepts values: `eye_dialect`, `partial`, `full`, `detailed`, `coarse`, `checked`, `anonymized`. Any other value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

## Notes

This is a warning-level diagnostic. Unsupported transcription values are preserved in the model for roundtrip fidelity.
