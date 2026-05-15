# E311: Failed to parse utterance

## Description

Failed to parse utterance

## Metadata
- **Status**: not_implemented
- **Status note**: Unreachable via tree-sitter parser. E311 (UnexpectedNode) is emitted by `chat_file_parser/single_item/helpers.rs` and `utterance_parser.rs`, but tree-sitter's error recovery wraps malformed utterance content in ERROR nodes that surface as E316 (UnparsableContent) before the unexpected-node check runs. The nested/unclosed bracket example `[: unclosed replacement [* error] .` is absorbed into an ERROR node, producing E316.

- **Error Code**: E311
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E3xx_main_tier_errors/E311_failed_parse_utterance.cha`
**Trigger**: Severely malformed utterance that parser cannot handle
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	[: unclosed replacement [* error] .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
