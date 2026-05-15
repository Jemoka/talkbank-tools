# E543: Header out of canonical order

## Description

A header appears out of canonical order. For example, `@Options` or `@ID` appears before `@Participants`. CHAT headers must follow the canonical ordering: `@UTF8`, `@Begin`, `@Languages`, `@Participants`, then other headers like `@Options` and `@ID`.

## Metadata

- **Error Code**: E543
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1

**Expected Error Codes**: E543

```chat
@UTF8
@Begin
@Languages:	eng
@Options:	CA
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;06.|male|||Target_Child|||
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E543 — `@Options` appears before `@Participants`, violating canonical header order

## CHAT Rule

Headers must appear in canonical order. `@Participants` must precede `@Options`, `@ID`, and other metadata headers. Only `@UTF8`, `@Begin`, and `@Languages` may appear before `@Participants`.

Reference: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

This diagnostic helps ensure consistent header ordering across CHAT files, matching the convention expected by CLAN tools.
