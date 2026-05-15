# E251: Empty word content text

## Description

A word content text segment (the spoken text portion of a word or the text
inside a shortening) is empty. The validator reports E251 when a `Text` or
`ShorteningText` element validates to empty via its inner `NonEmptyText`
wrapper.

**Validation not yet implemented for this spec example.** The example uses
`@s:eng .` which the parser treats as a valid language-tagged special form
marker, not as a word with an empty text content segment. The
`EmptyWordContentText` check fires on `WordContent::Text` and
`WordContent::ShorteningText` elements with empty inner text, but the parser
does not produce these from the example.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E251
- **Category**: validation
- **Level**: word
- **Layer**: parser

## Example 1

**Source**: `error_corpus/validation_errors/E251_empty_word_content_text.cha`
**Trigger**: Word with annotations but empty text
**Expected Error Codes**: E251

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	@s:eng .
@End
```

## Expected Behavior

The validator should report E251 when a word content text segment is empty.
The check exists in `crates/talkbank-model/src/model/content/word/content.rs`
via field-level validation with `ErrorCode::EmptyWordContentText`.

**Trigger conditions**: A `WordContentText` or `ShorteningText` whose inner
`NonEmptyText` validates to empty. This may only be constructible
programmatically, since the parser typically does not produce empty text
segments.

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT
manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Validation logic exists in `content.rs` as a model-level invariant check
- May only be triggerable programmatically; the parser avoids producing empty
  text segments
- The code IS emitted in the codebase via validate trait on `WordContentText`
