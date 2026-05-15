# E232: Compound marker at word start

**Last updated:** 2026-04-04 08:15 EDT

## Description

Compound marker (`+`) cannot be at the start of a word. Valid compounds have the form `left+right`.

## Metadata
- **Status**: implemented

- **Error Code**: E232
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E232_compound_marker_at_start.cha`
**Trigger**: Leading `+` in word — tree-sitter absorbs into ERROR node before word validation runs
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Compound marker (+) cannot be at word start
@Comment:	Invalid: '+hello' - Compound marker at start
*CHI:	+hello .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- This error code is emitted during word validation (model layer) and cannot currently be triggered by standalone CHAT input. Tree-sitter's grammar treats `+word` as a linker pattern or ERROR node, producing E316 before word-level validation can run. The E232 check exists in `word_validate.rs` and fires when a `Word` model object has a leading compound marker, but the parser never constructs such a word from `+hello` input.
- Review and enhance this specification as needed
