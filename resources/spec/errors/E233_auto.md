# E233: Empty compound part

## Description

Compound markers (`+`) must connect two non-empty parts. Adjacent compound markers
(`un++do`) create an empty part between them, which is invalid.

Note: Trailing `+` on a word (e.g., `hello+`) is structurally prevented by the
grammar — the `+` cannot be the last character of a word token, so it splits
into a separate token. This error covers internal empty parts only.

## Metadata

- **Error Code**: E233
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E233_empty_compound_part.cha`
**Trigger**: Adjacent compound markers create an empty part
**Expected Error Codes**: E233

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Adjacent compound markers create empty compound part
@Comment:	Invalid: 'un++do' - Empty part between compound markers
*CHI:	un++do .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: Adjacent compound markers within a word create an empty compound part.

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Trailing `+` on a word is now prevented by the grammar (standalone_word regex
  cannot end with `+`), so the original `hello+` example no longer applies.
- Internal empty parts (adjacent `++`) are still reachable via word-level validation.
