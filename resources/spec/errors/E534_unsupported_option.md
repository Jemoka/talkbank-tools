# E534: Unsupported @Options Value

## Description

An `@Options` header contains a flag that is not one of the recognized option values. The file parses successfully but the unsupported flag is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E534
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
@Options:	badoption
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E534 — unsupported @Options value 'badoption'

## CHAT Rule

The `@Options` header accepts only recognized option flags: `CA`, `dummy`, `NoAlign`. Any other value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>

## Notes

This is a warning-level diagnostic. Unsupported option values are preserved in the model as `Unsupported(String)` for roundtrip fidelity but flagged so users can correct them.
