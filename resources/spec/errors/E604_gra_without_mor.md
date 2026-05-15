# E604: %gra Tier Without %mor Tier

## Description

A %gra (grammatical relations) tier appears without a corresponding %mor (morphology) tier. According to CHAT rules, %gra depends on %mor and cannot exist independently.

## Metadata

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
%gra:	1|2|SUBJ 2|0|ROOT
@End
```

## Expected Behavior

- **Parser**: Should succeed - syntax is valid for both tiers
- **Validator**: Should report E604 - %gra tier present but %mor tier is missing

## CHAT Rule

The %gra tier provides grammatical relations derived from the %mor tier analysis. Every utterance with a %gra tier must also have a %mor tier that precedes it.

## Notes

This is a dependency validation error. The %gra tier references morphological units from the %mor tier, so %mor must be present first. This error is detected during semantic validation after successful parsing.
