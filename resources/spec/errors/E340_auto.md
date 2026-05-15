# E340: UnknownBaseContent

## Description

Main tier content could not be classified as any known word or construct
type. This fires when a `base_content_item` CST node has a child kind
that the Rust parser doesn't recognize — indicating a grammar/parser
mismatch (the grammar produces a new node type that the parser hasn't
been updated to handle).

## Metadata

- **Error Code**: E340
- **Category**: parser\_recovery
- **Level**: utterance
- **Layer**: parser
- **Status**: implemented

## Notes

- This error indicates a grammar/parser mismatch, not a CHAT input error.
- It cannot be triggered by any CHAT input with the current grammar; it
  would only fire if the grammar added a new `base_content_item` variant
  without updating the Rust parser.
- No example is possible with the current grammar.
