# E352: MissingQuoteEnd

## Description

A self-completion linker (`+,`) was used and there IS a prior utterance
from the same speaker, but that prior utterance did not end with a `+/.`
(interruption) terminator.

## Metadata
- **Status**: not_implemented
- **Layer**: validation

- **Error Code**: E352
- **Category**: cross\_utterance
- **Level**: utterance
- **Layer**: validation

## Example 1

**Trigger**: +, self-completion but prior same-speaker utterance ends with "."

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||male|||Target_Child|||
*CHI:	I want a cookie .
*CHI:	+, actually ice cream .
@End
```

## Expected Behavior

Validation should report E352. The `+,` linker signals self-completion,
which requires the prior same-speaker utterance to have been interrupted
(`+/.`). But the prior CHI utterance ends with `.` (a normal terminator).

## CHAT Rule

The `+,` linker pairs with the `+/.` terminator. The prior same-speaker
utterance must end with `+/.` to indicate it was interrupted, and the
`+,` utterance completes it.
