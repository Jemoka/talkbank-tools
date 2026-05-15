# E503: Missing required @UTF8 header

## Description

Every valid CHAT file must begin with an `@UTF8` header as its first line. This error indicates the file is missing `@UTF8`, which means the file's character encoding is unspecified. All modern CHAT files are expected to be UTF-8 encoded.

## Metadata

- **Error Code**: E503
- **Category**: Header validation
- **Level**: header
- **Layer**: validation

## Example 1: File without @UTF8

**Trigger**: File lacks `@UTF8` header entirely
**Expected Error Codes**: E503

```chat
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report E503 pointing at the **end of the file** (consistent with other missing-header errors like E502 and E504). The suggestion should instruct the user to add `@UTF8` as the first line.

## CHAT Rule

Every CHAT file must begin with `@UTF8` on the first line, followed by `@Begin`. See CHAT manual section on file structure: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- All 340 reference corpus files contain `@UTF8`, so this validation rule has zero impact on the roundtrip gate
- The grammar intentionally accepts files without `@UTF8` (parse-lenient), and this validation rule closes the gap (validate-strict)
- This fills the E503 slot in the E5xx sequence between E502 (MissingEndHeader) and E504 (MissingRequiredHeader)
