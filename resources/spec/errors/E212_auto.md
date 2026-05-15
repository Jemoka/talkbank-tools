# E212: Invalid word format

## Description

A word on the main tier has an invalid format that does not match any recognized
CHAT word structure. The validator reports E212 for specific structural
violations such as CA omissions used outside CA mode, CA omissions without
spoken text, or standalone shortenings.

**Validation not yet implemented for this spec example.** The example uses
`hello world .` which is perfectly valid CHAT. The `InvalidWordFormat` check in
`word_validate.rs` fires for specific structural issues (CA omissions outside CA
mode, malformed CA omissions, standalone shortenings) that this example does not
exhibit.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT
- **Layer**: validation

- **Error Code**: E212
- **Category**: Word validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `E2xx_word_errors/E212_unexpected_text.cha`
**Trigger**: Malformed word syntax caught by parser
**Expected Error Codes**: E212

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This is a parser error, hard to trigger with valid grammar
*CHI:	hello world .
@End
```

## Expected Behavior

The validator should report E212 for words with structurally invalid format.
The check exists in `crates/talkbank-model/src/model/content/word/word_validate.rs`
and fires for: CA omission `(word)` used outside CA mode, CA omissions without
spoken text or containing shortenings, and standalone shortening-only words.

**Trigger conditions**: CA omission outside `@Options: CA`, malformed CA
omission content, or a word consisting solely of a shortening marker.

## CHAT Rule

See CHAT manual sections on word-level syntax. The CHAT manual is available at:
https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Validation logic exists in `word_validate.rs` but the example does not trigger it
- The example contains valid CHAT and cannot produce E212
- A proper example would need `@Options: CA` context or specific word structures
