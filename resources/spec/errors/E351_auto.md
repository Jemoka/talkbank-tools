# E351: MissingQuoteBegin

## Description

A self-completion linker (`+,`) was used but there is no prior utterance
from the same speaker. The `+,` linker requires a preceding interrupted
utterance from the same speaker to complete.

## Metadata
- **Status**: not_implemented
- **Layer**: validation

- **Error Code**: E351
- **Category**: cross\_utterance
- **Level**: utterance
- **Layer**: validation

## Example 1

**Trigger**: +, self-completion as the very first utterance from this speaker

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||male|||Target_Child|||
*CHI:	+, hello .
@End
```

## Expected Behavior

Validation should report E351. The `+,` self-completion linker expects a
prior interrupted utterance from the same speaker (CHI), but this is the
first CHI utterance.

## CHAT Rule

The `+,` linker signals self-completion — the speaker continues their own
interrupted utterance. There must be a prior utterance from the same
speaker ending with `+/.` (interruption terminator).
