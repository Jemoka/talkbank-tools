# E376: Replacement parse error

## Description

Failed to parse replacement annotation content. The `[:` replacement
annotation contains content that cannot be parsed as valid replacement
words.

## Metadata

- **Error Code**: E376
- **Category**: Word validation
- **Level**: utterance
- **Layer**: parser
- **Status**: implemented

## Example 1

**Trigger**: Replacement with empty corrected form

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [:] world .
@End
```

## Expected Behavior

The parser should report E376 because the replacement annotation `[:]`
has no content — the corrected form after `[:` is empty.

## CHAT Rule

Replacement annotations use the form `word [: corrected_word]` where
the corrected form must contain at least one valid word. See the CHAT
manual: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus, then corrected.
- The original auto-generated spec incorrectly referenced E208.
- **Status note**: The example above triggers E208 (EmptyReplacement)
  rather than E376. The parser handles empty `[:]` via the E208 path.
  E376 (ReplacementParseError) fires for non-empty but malformed replacement
  content, but current tree-sitter error recovery routes those cases through
  E316 instead.
