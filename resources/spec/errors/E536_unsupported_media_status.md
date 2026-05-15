# E536: Unsupported @Media Status

## Description

An `@Media` header contains a status value that is not one of the recognized values. The file parses successfully but the unsupported status is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E536
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
@Media:	recording, audio, badstatus
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E536 — unsupported @Media status 'badstatus'

## CHAT Rule

The `@Media` header accepts status values: `unlinked`, `missing`, `notrans`. Any other status value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>

## Notes

This is a warning-level diagnostic. Unsupported media status values are preserved in the model for roundtrip fidelity.
