# E220 — Illegal digits in word content

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

A word on the main tier contains numeric digits in a language context that does
not permit them. Most natural languages (English, Spanish, French, etc.) do not
allow bare digits in words on the main tier. A small set of languages (Chinese,
Welsh, Vietnamese, Thai, Cantonese, etc.) permit digits as part of tone
notation or numerals.

## Metadata

- **Error Code**: E220
- **Category**: Word validation
- **Level**: word
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: English word containing a digit
**Expected Error Codes**: E220

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello3 .
@End
```

## Example 2

**Trigger**: Word with digits in Spanish (also not digit-allowing)
**Expected Error Codes**: E220

```chat
@UTF8
@Begin
@Languages:	spa
@Participants:	CHI Target_Child
@ID:	spa|corpus|CHI|||||Target_Child|||
*CHI:	hola2 .
@End
```

## Expected Behavior

Validation should report E220 on words containing digits when the language does
not allow them. The fix is either to spell out the number or use the
appropriate CHAT notation for the content.

## CHAT Rule

<https://talkbank.org/0info/manuals/CHAT.html>

Languages that allow digits in words: `zho`, `cym`, `vie`, `tha`, `nan`,
`yue`, `min`, `hak`. All other languages flag digits as E220.

## Notes

- Omission words (`0word`) are exempt — the `0` prefix is valid CHAT notation.
- For mixed/ambiguous language markers (`@s:eng+zho`), digits are allowed if
  ANY candidate language permits them.
- The digit-allowing language list is defined in
  `validation/context.rs::LANGUAGES_ALLOWING_NUMBERS`.
