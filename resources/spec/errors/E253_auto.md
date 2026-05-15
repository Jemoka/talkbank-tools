# E253: Empty word content

## Description

A parsed Word object has empty content — the word node exists in the CST but contains no text.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E253
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E253_empty_word_content.cha`
**Trigger**: Utterance with only whitespace before terminator — triggers E305 instead
**Expected Error Codes**: E305

Note: The example produces E305 (MissingTerminator / empty utterance detected
at the main tier level) rather than E253 (EmptyWordContent, a word-level
validation). E253 fires when a parsed Word object has empty content, which
requires the parser to produce a word node with no text — a condition that
does not arise from this input.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Word must have content
@Comment:	Invalid: Empty word element
*CHI:	  .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
