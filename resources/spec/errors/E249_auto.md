# E249 — Bare `@s` shortcut with no secondary language

**Status:** Current
**Last updated:** 2026-04-04 07:36 EDT

## Description

The `@s` shortcut means "the other language" — it toggles between the primary
and secondary language declared in `@Languages`. When there is **no secondary
language** (the `@Languages` header lists only one language), `@s` has no
target to resolve to. The speaker must use an explicit language code
(`@s:spa`, `@s:zho`, etc.) or add a second language to the `@Languages` header.

## Metadata

- **Error Code**: E249
- **Category**: validation
- **Level**: word
- **Layer**: validation
- **Status**: implemented

## Example 1

A monolingual file (`@Languages: eng`) uses the bare `@s` shortcut. There is
no secondary language, so `@s` cannot resolve.

**Expected Error Codes**: E249

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello@s .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file. Validation should report
E249 on the word `hello@s` because the `@s` shortcut cannot resolve when only
one language is declared. The fix is either:

1. Add a second language to the `@Languages` header (e.g., `@Languages: eng, spa`), or
2. Use an explicit language code on the word (e.g., `hello@s:spa`).

## CHAT Rule

The `@s` shortcut is defined in the CHAT manual:
<https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

> If the file's `@Languages` header lists two languages, `@s` without an
> explicit code refers to whichever of those two languages is NOT the current
> tier's language. If only one language is declared, there is no "other"
> language for `@s` to refer to.

## Notes

- This is a serious validation error: `word@s` in a monolingual file is
  semantically meaningless because there is no secondary language to toggle to.
- The error message suggests both fixes: adding a language to `@Languages` or
  using an explicit code.
- Resolution falls back to the tier language so downstream validation can
  continue, but the error is always emitted.
- Common in corpora where a file was originally bilingual and later had its
  `@Languages` header simplified without updating all word markers.
