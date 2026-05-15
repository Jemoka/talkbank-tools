# E545: `@Birth of` date does not match a legal CHAT date pattern

## Description

An `@Birth of <CODE>` header must carry a date matching CLAN's
authoritative `depfile.cut` date template:

```
@Birth of #:	@d<dd-lll-yyyy>
```

That is, the value must be `DD-MMM-YYYY`:

- `DD` — two-digit day, 01–31
- `MMM` — three-letter uppercase month abbreviation (JAN..DEC)
- `YYYY` — four-digit year

Anything else — lowercase month, two-digit year, numeric month,
free text — is rejected by CLAN CHECK, and Rust chatter must match.

Rust chatter historically did not validate `@Birth of` at all; the
date was stored as a raw string on `Participant.birth_date`
without any format check. This was a depfile.cut conformance gap.

## Metadata

- **Status**: implemented
- **Status note**: New validation added 2026-04-21 to close the
  `@Birth of` gap discovered during the depfile.cut conformance
  audit. Reuses the same date-format logic as `@Date` (E518) since
  depfile.cut gives both headers the same `@d<dd-lll-yyyy>` template.
- **Last updated**: 2026-04-21 23:55 EDT
- **Error Code**: E545
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1 (lowercase month)

**Trigger**: `@Birth of CHI: 15-jan-2024` — month must be uppercase.

**Expected Error Codes**: E545

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Birth of CHI:	15-jan-2024
*CHI:	hello .
@End
```

## Example 2 (two-digit year)

**Trigger**: `@Birth of CHI: 15-JAN-24` — year must be four digits.

**Expected Error Codes**: E545

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Birth of CHI:	15-JAN-24
*CHI:	hello .
@End
```

## Example 3 (numeric month)

**Trigger**: `@Birth of CHI: 15-01-2024` — month must be a
three-letter abbreviation, not numeric.

**Expected Error Codes**: E545

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Birth of CHI:	15-01-2024
*CHI:	hello .
@End
```

## Example 4 (free text)

**Trigger**: `@Birth of CHI: last Tuesday`.

**Expected Error Codes**: E545

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Birth of CHI:	last Tuesday
*CHI:	hello .
@End
```

## Expected Behavior

- **Parser**: accepts any non-empty string as the date value.
- **Validator**: reports E545 at the header span when the value
  does not match the canonical `DD-MMM-YYYY` form. The check
  reuses the same code as `@Date` (`E518`), which already
  enforces the day/month/year rules granularly.

## CHAT Rule

CLAN `depfile.cut`:

```
@Birth of #:	@d<dd-lll-yyyy>
```

CHAT manual: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

- Authoritative source: `clan-info/lib/depfile.cut:23`.
- Severity: `Error` (depfile rejections are errors in CLAN CHECK).
- Implementation reuses `check_date_format` from `metadata.rs`;
  the difference from `@Date` is only the error code emitted.
