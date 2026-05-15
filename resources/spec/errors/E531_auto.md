# E531: Media filename mismatch

## Description

The filename in the `@Media` header does not match the name of the CHAT file
being parsed (case-insensitive comparison). For example, if `foo.cha` contains
`@Media: bar, audio`, E531 is reported because `bar` does not match `foo`.

**Validation not yet implemented for this spec example.** The check in
`crates/talkbank-model/src/model/file/chat_file/validate.rs` compares the
`@Media` filename against the file's own name, but only when a filename is
provided to the validator (via `validate(&errors, Some(filename))`). The spec
test infrastructure may not pass a filename during validation, so the check
is skipped. The example itself is correct — `@Media: different, audio` in a
file not named `different.cha` should trigger E531.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E531
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E531_media_filename_mismatch.cha`
**Trigger**: Media filename does not match the CHAT file name
**Expected Error Codes**: E531

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	different, audio
@Comment:	ERROR: Media filename must match transcript
*CHI:	hello .
@End
```

## Expected Behavior

The validator should report E531 when the `@Media` header filename does not
match the CHAT file name (case-insensitive). The check exists in
`crates/talkbank-model/src/model/file/chat_file/validate.rs`.

**Trigger conditions**: `@Media` header contains a filename that differs from
the stem of the `.cha` file being validated. The validator must be invoked with
the file path to enable this check.

## CHAT Rule

See CHAT manual on the `@Media` header. The media filename should match the
transcript filename. The CHAT manual is available at:
https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Validation logic exists and is correct, but requires the filename to be
  passed to the validator at invocation time
- The spec test runner may not provide the filename context needed to trigger
  the check
- The code IS emitted in production when files are validated via CLI with paths
