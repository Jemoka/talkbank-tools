# E242: Unbalanced quotation marks

**Last updated:** 2026-04-04 08:15 EDT

## Description

Quotation marks must be balanced within an utterance.

## Metadata
- **Status**: implemented

- **Error Code**: E242
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E242_unbalanced_quotation.cha`
**Trigger**: Unbalanced opening quote — tree-sitter absorbs into ERROR node before quotation validation runs
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Quotation marks must be balanced
@Comment:	Invalid: '"hello' - Missing closing quote
*CHI:	"hello .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- This error code is emitted during utterance quotation validation (model layer) and cannot currently be triggered by standalone CHAT input. Tree-sitter's grammar cannot parse an unbalanced `"hello` and produces E316 (generic unparsable content) before quotation validation can run. The E242 check exists in `quotation.rs` and fires when the model detects unmatched quotation begin/end markers.
- Review and enhance this specification as needed
