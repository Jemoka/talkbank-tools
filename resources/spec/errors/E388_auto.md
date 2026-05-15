# E388: Replacement on non-word

## Description

Replacement annotation `[: ...]` is attached to a non-word element (e.g., a paralinguistic event like `&=laugh`), which cannot be replaced.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E388
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `E2xx_word_errors/E388_replacement_on_nonword.cha`
**Trigger**: Replacement annotation on a paralinguistic event (non-word)
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&=laugh [: well] .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
