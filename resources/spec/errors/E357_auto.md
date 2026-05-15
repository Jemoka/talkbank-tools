# E357: UnmatchedUnderlineEnd

## Description

An underline end marker was found without a preceding underline begin
marker in the same utterance. The end marker has no open underline to
close.

## Metadata
- **Status**: implemented
- **Layer**: validation

- **Error Code**: E357
- **Category**: underline\_balance
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E357_unmatched_underline_end.cha`
**Trigger**: Underline end marker (`\x02\x02`) without preceding begin marker (`\x02\x01`)
**Expected Error Codes**: E357

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Unmatched underline end marker
*CHI:	hello world .
@End
```

## Expected Behavior

Validation should report E357. The underline end control character
(`\x02\x02`) appears without a preceding begin character (`\x02\x01`)
on the stack.

## Notes

- Underline markers are control characters used in CA (Conversation
  Analysis) transcription. An orphaned end marker without a matching
  begin is a data error.
