# E208: Empty replacement

## Description

Empty replacement

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-13 23:00 EDT

- **Error Code**: E208
- **Category**: validation
- **Level**: word
- **Layer**: parser

## Example 1

**Source**: `error_corpus/validation_errors/E208_empty_replacement.cha`
**Trigger**: Replacement with empty target
**Expected Error Codes**: E376

Note: The parser produces E376 (ReplacementParseError) for `[: ]` because
tree-sitter's error recovery inserts a placeholder node for the missing word.
E208 (EmptyReplacement) exists in model validation but requires the parser to
successfully build a Replacement with zero words, which does not currently
happen for this input.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello [: ] .
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
