# E209 — Word has no spoken content

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

A word on the main tier consists entirely of shortening notation `(text)` with
no actual spoken material. In CHAT, `(the)` means the sounds were omitted — it
is not the same as the word being spoken. To mark an omitted word, use
`0the` (zero-word) instead.

This is the Rust equivalent of CLAN CHECK error 155.

## Metadata

- **Error Code**: E209
- **Category**: validation
- **Level**: word
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: Standalone shortening `(the)` as entire word — no spoken material
**Expected Error Codes**: E209

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	(the) dog .
@End
```

## Expected Behavior

Validation should report E209 on `(the)` because the word has no spoken
content — only a shortening marker. The fix is to use `0the` (zero-word
notation) instead of `(the)`.

## CHAT Rule

See CHAT manual on shortening and zero-words:
<https://talkbank.org/0info/manuals/CHAT.html>

Shortening `(text)` represents omitted sounds within a spoken word (e.g.,
`(be)cause`). A standalone shortening with no surrounding spoken material is
semantically empty — the speaker produced no sound for this word.

## Notes

- `has_spoken_material()` checks for `WordContent::Text` elements. Shortening
  elements are deliberately excluded because they represent omitted sounds.
- `cleaned_text()` DOES include shortening content, so `(the)` has non-empty
  cleaned text but fails the spoken-material check.
- See `docs/spoken-content-analysis.md` for the full analysis of spoken vs
  non-spoken content in the word model.
