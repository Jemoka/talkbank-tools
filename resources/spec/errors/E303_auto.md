# E303: Unexpected node - helper function

## Description

Unexpected node - helper function

## Metadata
- **Status**: not_implemented
- **Status note**: Unreachable via tree-sitter parser for this example. E303 (SyntaxError) is emitted by `error_analysis/file.rs` and by the re2c parser, but the example's missing `@UTF8`/`@Participants`/`@ID` scaffolding triggers header-level errors (E501-E505) before the syntax-error path is reached. Tree-sitter's error recovery also absorbs `{{{ world }}}` into ERROR nodes routed through other codes (E316/E375).

- **Error Code**: E303
- **Category**: Parser bugs (experimental)
- **Level**: utterance
- **Layer**: parser

## Example

```chat
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	hello {{{ world }}} .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: Try to trigger internal parser bug with unexpected parse node

## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or \!). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus file: `error_corpus/E3xx_main_tier_errors/E331_unexpected_node_helper.cha`
- Review and enhance this specification as needed
