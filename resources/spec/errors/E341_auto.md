# E341: UnbalancedQuotationCrossUtterance

## Description

A quotation-follows terminator (`+"/.`) is used but the next utterance
from the same speaker does not begin with a quotation precedes linker
(`+"`). This indicates an unbalanced cross-utterance quotation sequence.

## Metadata
- **Status**: not_implemented
- **Layer**: validation

- **Error Code**: E341
- **Category**: cross\_utterance
- **Level**: utterance
- **Layer**: validation

## Example 1

**Trigger**: +"/. terminator but next same-speaker utterance lacks +" linker

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	MOT Mother, CHI Target_Child
@ID:	eng|test|MOT||female|||Mother|||
@ID:	eng|test|CHI||male|||Target_Child|||
*MOT:	she said +"/.
*CHI:	what .
*MOT:	go to bed .
@End
```

## Example 2

**Trigger**: +"/. terminator with no subsequent same-speaker utterance

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	MOT Mother
@ID:	eng|test|MOT||female|||Mother|||
*MOT:	she said +"/.
@End
```

## Expected Behavior

Validation should report E341. The `+"/.` terminator opens a quotation
that requires the next same-speaker utterance to begin with `+"`.

## CHAT Rule

The quotation follows convention: `+"/.` signals that the next utterance
from the same speaker is a quoted utterance and must begin with `+"`.
See the CHAT manual on quotation conventions.
