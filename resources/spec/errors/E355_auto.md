# E355: InterleavedScopedAnnotations

## Description

An other-completion linker (`++`) was used but the preceding utterance is
from the **same** speaker. The `++` linker is for other-completion
(completing a different speaker's utterance). To complete one's own
utterance, use `+,` (self-completion) instead.

## Metadata
- **Status**: not_implemented
- **Layer**: validation

- **Error Code**: E355
- **Category**: cross\_utterance
- **Level**: utterance
- **Layer**: validation

## Example 1

**Trigger**: ++ other-completion but preceding utterance is from the same speaker

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||male|||Target_Child|||
*CHI:	I was going +...
*CHI:	++ to say hello .
@End
```

## Expected Behavior

Validation should report E355. Both utterances are from CHI. The `++`
linker is meant for other-completion (Speaker B completing Speaker A),
but here CHI is completing their own utterance. The correct linker
would be `+,` (self-completion).

## Notes

- Despite the variant name "InterleavedScopedAnnotations", this error
  is actually about same-speaker other-completion usage.
