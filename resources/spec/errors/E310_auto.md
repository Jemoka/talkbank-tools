# E310: Parser failed to produce valid parse tree

## Description

Tree-sitter's internal parser returned `None` (e.g., due to timeout or
cancellation) or the parse outcome was rejected with no other errors collected.
E310 is a catch-all for complete parse failures where no more specific error
code applies.

**Validation not yet implemented for this spec example.** The example is a
valid CHAT file (headers only, no utterances) which parses successfully.
E310 fires when tree-sitter itself fails to produce a parse tree, which
requires genuinely unparseable input or a parser timeout — not merely
missing utterances.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT
- **Layer**: validation
- **Status note**: Unreachable via tree-sitter parser for any parseable input. E310 (ParseFailed) fires only when tree-sitter's `parse()` returns `None` (e.g., timeout or cancellation) or when the parse outcome is `Rejected` with an empty error list. Tree-sitter's error recovery produces a CST for essentially any textual input, so this path cannot be exercised from a CHAT fragment. The example parses successfully as a headers-only file and produces no errors.

- **Error Code**: E310
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E310_failed_parse_headers.cha`
**Trigger**: Malformed header structure
**Expected Error Codes**: E310

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
```

## Expected Behavior

The parser should report E310 when it cannot produce a valid parse tree at all.
The check exists in `crates/talkbank-parser/src/parser/chat_file_parser/chat_file/parse.rs`
and `helpers.rs`.

**Trigger conditions**: Tree-sitter returns `None` from `parse()`, or the parse
outcome is `Rejected` with an empty error list. This typically requires
severely malformed input that tree-sitter cannot recover from at all.

## CHAT Rule

A valid CHAT file must be parseable by the tree-sitter grammar. The CHAT manual
is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The example is a valid CHAT file with only headers and no utterances,
  which parses successfully
- E310 is difficult to trigger from well-formed CHAT text; it requires
  input that causes tree-sitter's parser to fail entirely
- The code IS emitted in the codebase as a fallback for total parse failure
