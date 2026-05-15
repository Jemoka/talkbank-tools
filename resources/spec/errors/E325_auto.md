# E325: UnexpectedUtteranceChild

## Description

An unexpected child node was found inside a parsed utterance. The CST
contains a node that is neither the main tier nor a recognized dependent
tier kind. This typically indicates a tree-sitter error recovery scenario
where an unusual node type ends up inside an utterance subtree.

## Metadata
- **Status**: not_implemented

- **Error Code**: E325
- **Category**: parser\_recovery
- **Level**: utterance
- **Layer**: parser

## Example 1

**Trigger**: A line between main tier and dependent tier that tree-sitter groups into the utterance

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI:	hello .
@@@unexpected content
%mor:	co|hello .
@End
```

## Expected Behavior

The parser should report E325 if tree-sitter places the unexpected content
as a child of the utterance node rather than as a separate line.

## Notes

- This error depends on tree-sitter's error recovery placing unusual nodes
  inside an utterance subtree. The exact trigger depends on the grammar's
  recovery behavior, making it difficult to trigger deterministically.
- Status is `implemented` because the trigger depends on parser internals.
