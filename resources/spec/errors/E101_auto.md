# E101: Invalid line format

**Last updated:** 2026-04-04 08:28 EDT

## Description

A line in the CHAT file does not match any valid line format (must start with
`@`, `*`, `%`, or be a continuation tab). E101 (InvalidLineFormat) is defined
as an error code but is not currently emitted by the tree-sitter parser. The
parser produces header validation errors for the missing scaffolding and does
not reach E101 detection.

## Metadata
- **Status**: not_implemented
- **Status note**: Unreachable via tree-sitter parser. E101 (InvalidLineFormat) is defined in the error code enum but is not emitted by any production code path; it is only referenced by the CLAN CHECK error mapping in `error_map.rs`. The tree-sitter parser routes unrecognizable lines through header validation (E501-E504) or more specific parser-recovery codes (E319, E320, E321). The example's missing `@UTF8`/`@Participants`/`@ID` scaffolding causes E501-E504 to fire instead.

- **Error Code**: E101
- **Category**: validation
- **Level**: file
- **Layer**: parser

## Example 1

**Source**: `error_corpus/parse_errors/E101_invalid_line_format.cha`
**Trigger**: File has `@Begin` and `@Languages` but is missing `@UTF8`,
`@Participants`, `@ID`, and has an invalid bare line `InvalidLine`.
**Expected Error Codes**: E501, E502, E503, E504

Note: E101 is not emitted by the parser. The example is missing required
headers (`@UTF8`, `@Participants`, `@ID`, `@End`), which produces header
validation errors instead. E501 fires for duplicate/structural header issues,
E502 for missing `@End`, E503 for missing `@UTF8`, E504 for missing required
headers.

```chat
@Begin
@Languages:	eng
InvalidLine
@Comment:	ERROR: Line format invalid
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See the CHAT manual for format specifications: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- E101 (InvalidLineFormat) is defined in the error code enum but is not
  currently emitted anywhere in the tree-sitter parser or validation pipeline.
  It is only referenced in the CLAN CHECK error mapping (`error_map.rs`).
- The example produces E501, E502, E503, E504 due to missing scaffolding
  headers, not E101.
