# E322: EmptyColon

## Description

The main tier speaker prefix has a zero-width (MISSING) colon node.
This occurs when tree-sitter synthesizes an empty colon placeholder
because the speaker code has no colon at all.

## Metadata
- **Status**: not_implemented

- **Error Code**: E322
- **Category**: parser\_recovery
- **Level**: utterance
- **Layer**: parser

## Example 1

**Trigger**: Speaker code without a colon separator

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI hello .
@End
```

## Expected Behavior

The parser should report E322. The `*CHI hello .` line is missing the
colon after the speaker code. Tree-sitter inserts a zero-width MISSING
colon node, which the parser detects.

## Notes

- Tree-sitter error recovery synthesizes a MISSING colon placeholder
  at `child.start_byte() == child.end_byte()`.
- **Status note**: The example above triggers E316 (generic parse error)
  rather than E322. Tree-sitter handles the missing colon through a different
  error recovery path. Triggering E322 requires tree-sitter to produce a
  zero-width MISSING colon node specifically.
