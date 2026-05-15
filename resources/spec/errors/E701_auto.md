# E701 — Per-speaker start-time not monotonically increasing

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

Each utterance's first media bullet must have a start time greater than or
equal to the previous utterance's first bullet start time (for the same
speaker). Corresponds to CLAN CHECK Error 83.

## Metadata

- **Error Code**: E701
- **Category**: Temporal validation
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: Same speaker's second utterance starts before the first
**Expected Error Codes**: E701

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello . 5000_6000
*CHI:	world . 3000_4000
@End
```

## Expected Behavior

Validation should report E701 on the second `*CHI:` utterance because its
bullet start time (3000ms) is less than the first utterance's start time
(5000ms). The timestamps must be monotonically increasing per speaker.

## CHAT Rule

<https://talkbank.org/0info/manuals/CHAT.html#Bullets>

## Notes

- Skipped in CA mode (`@Options: CA`) where timing constraints are relaxed.
- Implementation: `crates/talkbank-model/src/validation/temporal.rs`
- E704 (same-speaker overlap with 500ms tolerance) may also fire for the same
  input when overlap exceeds the tolerance threshold.
