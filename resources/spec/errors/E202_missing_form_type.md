# E202: Missing form type after @

## Description

A word contains `@` at a position where a form type marker is expected, but
no valid form type follows. Tree-sitter produces an ERROR node at the `@`.

Valid form types: `@b`, `@c`, `@d`, `@f`, `@fp`, `@g`, `@i`, `@k`, `@l`,
`@ls`, `@n`, `@o`, `@p`, `@q`, `@sas`, `@si`, `@sl`, `@t`, `@wp`, `@x`,
`@z`, `@s` (language marker).

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E202
- **Category**: Word validation
- **Level**: word
- **Layer**: parser

## Example 1

**Trigger**: Word ending with bare `@` — no form type follows
**Expected Error Codes**: E202

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello@ .
@End
```

## Example 2

**Trigger**: Word with `@` followed by invalid form letter
**Expected Error Codes**: E203

Note: `@j` is recognized as a form type syntactically, but `j` is not a valid
form type value. This triggers E203 (InvalidFormType) rather than E202
(MissingFormType).

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	dog@j .
@End
```

## Expected Behavior

The parser should report E202 and recover by treating the word as malformed.
The raw text is preserved for downstream tools that may handle it differently.
