# E537: Unsupported @Number Value

## Description

An `@Number` header contains a value that is not one of the recognized number options. The file parses successfully but the unsupported value is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E537
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
@Number:	badnumber
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E537 — unsupported @Number value 'badnumber'

## CHAT Rule

The `@Number` header accepts values: `1`, `2`, `3`, `4`, `5`, `more`, `audience`. Any other value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>

## Notes

This is a warning-level diagnostic. Unsupported number values are preserved in the model for roundtrip fidelity.
