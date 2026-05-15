# E542: Unsupported @ID Sex Value

## Description

An `@ID` header contains a sex field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E542
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;06.|badsex|||Target_Child|||
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E542 — unsupported @ID sex value 'badsex'

## CHAT Rule

The `@ID` header sex field accepts values: `male`, `female`. Any other value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

## Notes

This is a warning-level diagnostic. Unsupported sex values are preserved in the model for roundtrip fidelity.
