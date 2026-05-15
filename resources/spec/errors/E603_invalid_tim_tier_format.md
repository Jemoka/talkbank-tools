# E603: Invalid %tim Tier Format

## Description

A `%tim` dependent tier contains content that does not match the expected time format. The tier parses successfully but the invalid content is stored as `Unsupported` and flagged during validation.

## Metadata

- **Error Code**: E603
- **Category**: tier_validation
- **Level**: utterance
- **Layer**: validation

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world .
%tim:	afternoon session
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E603 — invalid %tim tier format 'afternoon session'

## CHAT Rule

The `%tim` tier provides timing information for an utterance. Content should be in time format: `HH:MM:SS` or `HH:MM:SS.mmm`, or a time range.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

## Notes

This is a warning-level diagnostic. Invalid %tim content is preserved in the model as `Unsupported` for roundtrip fidelity. The `TimTier` type parses valid time formats into structured data and falls back to `Unsupported` for non-time content.
