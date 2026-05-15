# E546: Unsupported @ID SES Value

## Description

An `@ID` header contains an SES (socioeconomic status) field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as `Unsupported(String)` and flagged during validation.

## Metadata

- **Error Code**: E546
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;06.|female||badses|Target_Child|||
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E546 — unsupported @ID SES value 'badses'

## CHAT Rule

The `@ID` header SES field accepts values: ethnicity codes (`White`, `Black`, `Asian`, `Latino`, `Pacific`, `Native`, `Multiple`, `Unknown`), SES codes (`UC`, `MC`, `WC`, `LI`), or combined with comma or space separator (`White,MC` or `White MC`). Any other value is flagged as unsupported. Vocabulary source: `depfile.cut`.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

## Notes

This is a warning-level diagnostic. Unsupported SES values are preserved in the model for roundtrip fidelity.
