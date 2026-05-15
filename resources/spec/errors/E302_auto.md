# E302: Missing required node

**Last updated:** 2026-04-04 08:28 EDT

## Description

Expected tree-sitter node is missing. E302 (MissingNode) fires when
tree-sitter's error recovery inserts a MISSING placeholder node, indicating
the grammar expected a specific construct that was not found. This is an
internal parser condition triggered by tree-sitter error recovery, not by
specific CHAT syntax patterns. It also fires in speaker code validation for
invalid characters.

## Metadata
- **Status**: not_implemented

- **Error Code**: E302
- **Category**: validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `error_corpus/parse_errors/E302_missing_node.cha`
**Trigger**: Missing `@UTF8` and `@End` headers; lowercase speaker code `*ch:`
is not directly what triggers E302 — the missing headers dominate.
**Expected Error Codes**: E501, E502, E503, E504, E505

Note: E302 fires when tree-sitter inserts a MISSING node during error
recovery, which is difficult to trigger reliably from specific input. The
example is missing `@UTF8` and `@End` headers, so the parser produces header
validation errors (E501-E505) instead of E302. The lowercase speaker `*ch:`
would trigger E308/E522 (undeclared speaker) if the file had proper scaffolding.

```chat
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*ch:	hello .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or \!). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- E302 is emitted in two places: (1) `collect_tree_errors()` when
  `node.is_missing()` (tree-sitter error recovery), and (2) speaker code
  validation for invalid characters. Both are difficult to trigger reliably
  from crafted CHAT input because the grammar either fully parses or produces
  ERROR nodes (E316) rather than MISSING nodes.
- The example produces header validation errors due to missing scaffolding.
