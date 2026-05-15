# E309: Unexpected syntax

**Last updated:** 2026-04-04 08:28 EDT

## Description

Unexpected syntax encountered during parsing. E309 (UnexpectedSyntax) fires
when the parser encounters an ERROR node from tree-sitter that contains
unexpected content. The error is emitted from `make_error_from_node()` in
`helpers.rs`.

## Metadata
- **Status**: not_implemented

- **Error Code**: E309
- **Category**: validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `error_corpus/parse_errors/E309_unexpected_syntax.cha`
**Trigger**: `##` in utterance — intended to trigger unexpected syntax, but
the tree-sitter grammar silently accepts `##` as valid content. The missing
headers (`@UTF8`, `@End`) dominate the error output.
**Expected Error Codes**: E501, E502, E503, E504, E505

Note: E309 fires from ERROR nodes in tree-sitter's parse tree, but `##` in
an utterance does not produce an ERROR node — the grammar absorbs it. The
example also lacks `@UTF8` and `@End`, so header validation errors dominate.
With proper scaffolding and this specific input, the parser produces zero
errors (the `##` is silently accepted).

```chat
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello ## world .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or \!). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- E309 IS emitted by the parser (in `helpers.rs` for ERROR nodes), but the
  current example does not trigger it because `##` is not flagged by the
  tree-sitter grammar. A better example would need input that produces an
  ERROR node in the CST specifically within an utterance context.
- The example produces header validation errors due to missing `@UTF8` and
  `@End` scaffolding.
