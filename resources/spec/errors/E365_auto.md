# E365: Malformed tier content

## Description

A header or tier has content that does not match any recognized CHAT header
structure. The parser reports E365 when it encounters an unknown node type
during header dispatch in the CST.

**Validation not yet implemented for this spec example.** The example has a
`%pho` dependent tier with content `***bad***`. The `%pho` tier is parsed by
the dedicated phonology tier parser, not by the generic header dispatch that
emits E365. The `MalformedTierContent` check in `header_dispatch/parse.rs`
fires for unrecognized header node types in the CST, which requires a node
that does not match any known header pattern.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT
- **Layer**: validation
- **Status note**: Unreachable via tree-sitter parser for this example. E365 (MalformedTierContent) fires from `header_dispatch/parse.rs` when a CST node in the header/tier area has an unrecognized node type. The `%pho` tier in the example is a recognized tier type and dispatches to the dedicated phonology tier parser, so it never reaches the generic header-dispatch fallback. Triggering E365 requires a CST-level anomaly where a node type is not registered in the dispatch table — difficult to produce from textual input.

- **Error Code**: E365
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E365_malformed_tier_content.cha`
**Trigger**: Tier with unrecognizable content
**Expected Error Codes**: E365

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
%pho:	***bad***
@End
```

## Expected Behavior

The parser should report E365 when a header/tier node in the CST has an
unrecognized node type that does not match any known dispatch target. The check
exists in `crates/talkbank-parser/src/parser/chat_file_parser/header_dispatch/parse.rs`.

**Trigger conditions**: A CST node in the header/tier area with a node type
not matching any of the known header types (`@Languages`, `@Participants`,
`@ID`, etc.) or known tier types (`%mor`, `%gra`, `%pho`, etc.).

## CHAT Rule

See CHAT manual on dependent tiers and header format. The CHAT manual is
available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The `%pho` tier is a recognized tier type and gets its own parser,
  so malformed content within it produces different error codes
- E365 fires at the header dispatch level for entirely unrecognized node types
- The code IS emitted in the codebase but requires CST-level anomalies
