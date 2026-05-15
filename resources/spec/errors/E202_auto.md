# E202: Missing form type after @

## Description

Missing form type after @

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E202
- **Category**: Parser error
- **Level**: word
- **Layer**: parser

## Example 1

**Source**: `E2xx_word_errors/E202_empty_word.cha`
**Trigger**: @ symbol with no form type marker
**Expected Error Codes**: E202

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello@ world .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
