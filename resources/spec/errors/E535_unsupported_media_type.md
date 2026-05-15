# E535: Unsupported @Media Type

## Description

An `@Media` header contains a media type that is not one of the recognized values. The file parses successfully but the unsupported type is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E535
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
@Media:	recording, badtype
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E535 — unsupported @Media type 'badtype'

## CHAT Rule

The `@Media` header accepts media types: `audio`, `video`, `missing`. Any other type value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

## Notes

This is a warning-level diagnostic. Unsupported media types are preserved in the model for roundtrip fidelity.
