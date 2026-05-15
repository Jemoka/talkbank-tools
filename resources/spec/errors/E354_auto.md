# E354: MissingTrailingOffTerminator

## Description

An other-completion linker (`++`) was used and the preceding utterance is
from a different speaker, but that preceding utterance did not end with
`+...` (trailing off). The other-completion convention requires the
previous speaker to have trailed off.

## Metadata
- **Status**: not_implemented
- **Layer**: validation

- **Error Code**: E354
- **Category**: cross\_utterance
- **Level**: utterance
- **Layer**: validation

## Example 1

**Trigger**: ++ other-completion but prior different-speaker utterance ends with "."

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	MOT Mother, CHI Target_Child
@ID:	eng|test|MOT||female|||Mother|||
@ID:	eng|test|CHI||male|||Target_Child|||
*MOT:	go to bed .
*CHI:	++ no I won't .
@End
```

## Expected Behavior

Validation should report E354. CHI uses `++` (other-completion), which
requires MOT's preceding utterance to have ended with `+...` (trailing
off). But MOT's utterance ends with `.` (normal terminator).

## CHAT Rule

The `++` linker pairs with `+...`. Speaker B uses `++` to complete
Speaker A's trailed-off utterance. Speaker A must end with `+...`.
