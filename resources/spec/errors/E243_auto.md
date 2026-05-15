# E243: Illegal characters in word

## Description

Word contains illegal characters such as whitespace, control characters, or bullet markers that are not valid in word content.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E243
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E243_illegal_characters.cha`
**Trigger**: Word ending with bare `@` (no form type) — triggers E202 instead
**Expected Error Codes**: E202

Note: The example `hell@` triggers E202 (MissingFormType) rather than E243
(IllegalCharactersInWord) because the parser detects the bare `@` as a missing
form type marker. E243 fires at the validation layer on parsed words containing
whitespace, control characters, or bullet markers — conditions that are
difficult to reach via normal CHAT input.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: @ character only allowed for form markers
@Comment:	Invalid: 'hell@' - @ in wrong position
*CHI:	hell@ .
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
