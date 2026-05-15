# E344: Invalid scoped annotation nesting

**Last updated:** 2026-04-04 08:15 EDT

## Description

Invalid nesting of scoped annotations (quotation precedes pattern). This is a cross-utterance validator (`check_quotation_precedes`) that is currently DISABLED (`enable_quotation_validation: false`).

## Metadata
- **Status**: not_implemented

- **Error Code**: E344
- **Category**: validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `error_corpus/parse_errors/E344_invalid_scoped_nesting.cha`
**Trigger**: Overlapping scoped annotations — tree-sitter absorbs nested angle brackets into ERROR node
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello <world <foo> bar> .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or \!). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- This error code is emitted during cross-utterance quotation validation (`quotation_precedes.rs`) which is currently disabled (`enable_quotation_validation: false`). The spec example uses nested angle brackets which tree-sitter cannot parse, producing E316 instead. Even with a parseable example, the validator is disabled and would not fire.
- Review and enhance this specification as needed
