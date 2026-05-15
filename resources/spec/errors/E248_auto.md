# E248 — Bare `@s` shortcut in tertiary language context

**Status:** Current
**Last updated:** 2026-04-04 07:36 EDT

## Description

The bare `@s` shortcut toggles between the first two languages declared in
`@Languages`. When an utterance is scoped to a **tertiary** language (position
3 or later in the `@Languages` list) via `[- code]`, bare `@s` is ambiguous —
it could mean either the primary or secondary language. The speaker must use an
explicit code (`@s:eng`, `@s:spa`, etc.) instead.

## Metadata

- **Error Code**: E248
- **Category**: validation
- **Level**: word
- **Layer**: validation
- **Status**: implemented

## Example 1

A file declares three languages. The utterance is scoped to the third language
(`zho`) via `[- zho]`. Using bare `@s` is ambiguous — does it mean `eng` or
`spa`?

**Expected Error Codes**: E248

```chat
@UTF8
@Begin
@Languages:	eng, spa, zho
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	[- zho] ni3hao3@s .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file. Validation should report
E248 on the word `ni3hao3@s` because the `@s` shortcut cannot resolve when the
utterance language (`zho`) is tertiary. The fix is to use an explicit language
code: `ni3hao3@s:eng` or `ni3hao3@s:spa`.

## CHAT Rule

The `@s` shortcut is defined in the CHAT manual:
<https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

The `[- code]` utterance-scoped language marker is documented at:
<https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>

> The `@s` marker without an explicit language code toggles between the first
> and second language listed in `@Languages`. When more than two languages are
> declared, any word in a tertiary-scoped utterance must use `@s:LANGCODE`
> explicitly.

## Notes

- Three or more languages are common in bilingualism corpora (e.g., a child
  exposed to English, Spanish, and a heritage language).
- The error message suggests using `@s:eng` as a concrete fix example.
- Resolution falls back to the tier language so downstream validation can
  continue, but the error is always emitted.
