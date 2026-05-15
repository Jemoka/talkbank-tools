# E320: UnparsableHeader

## Description

A header line (starting with @) could not be parsed. This is a fallback
error emitted when tree-sitter produces an ERROR node in header context,
but the header type is not one of the specifically handled types
(@Participants, @Languages, @Date, @Media, @ID).

## Metadata
- **Status**: not_implemented

- **Error Code**: E320
- **Category**: parser\_recovery
- **Level**: utterance
- **Layer**: parser

## Example 1

**Trigger**: A malformed known header type (not @Participants/@Languages/@Date/@Media/@ID)

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
@Transcription:	[malformed content with [unbalanced brackets
*CHI:	hello .
@End
```

## Expected Behavior

The parser should report E320. The `@Transcription:` line begins with `@`
(header context) but its content causes tree-sitter to produce an ERROR
node. Since `@Transcription` is not one of the five specifically analyzed
header types, the fallback E320 fires.

## Notes

- More specific header errors exist: E505 (@ID), E506 (@Participants),
  E507 (@Languages), E508 (@Date), E509 (@Media).
- E320 is the catch-all for all other malformed headers.
- **Status note**: The example above triggers E316 (generic parse error)
  rather than E320. Tree-sitter's error recovery handles the malformed header
  content through a different path. Triggering E320 requires tree-sitter to
  produce an ERROR node specifically in header context for non-critical header
  types.
