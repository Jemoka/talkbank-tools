# E245 — Stress marker without following spoken material

**Status:** Current
**Last updated:** 2026-04-13 12:00 EDT

## Description

A primary stress marker (`ˈ`) or secondary stress marker appears at the start
of a word but is not followed by any spoken material. The marker has nothing
to attach to.

## Metadata

- **Error Code**: E245
- **Category**: validation
- **Level**: word
- **Layer**: parser
- **Status**: implemented

## Example 1

**Trigger**: Lone stress marker as entire word content
**Expected Error Codes**: E245

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈ .
@End
```

## Expected Behavior

The parser reports E245 on the word `ˈ` because after stripping the stress
marker there is no remaining spoken material for it to attach to. The word
is rejected (no dummy `Word` is fabricated) so downstream validation does
not see a zero-content word.

## CHAT Rule

See CHAT manual sections on stress markers and word-level syntax:
<https://talkbank.org/0info/manuals/CHAT.html>

## Notes

- Detection lives in the tree-sitter CST→model conversion for
  `standalone_word` (see `talkbank-parser` `word/mod.rs`). Triggered when
  the cleaned text (Text + Shortening items) is empty while the word body
  was non-empty in source.
- The regression test is
  `crates/talkbank-parser/tests/e245_stress_marker_regression.rs`.
