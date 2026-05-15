# E605: Unsupported Dependent Tier

## Description

An utterance contains a dependent tier with a label that is not a standard CHAT tier name and does not follow the `%x` user-defined tier naming convention. The file parses successfully but the tier is stored as `DependentTier::Unsupported` and flagged during validation.

## Metadata

- **Error Code**: E605
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
%foo:	unknown tier content
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E605 — unsupported dependent tier '%foo'

## CHAT Rule

Dependent tiers must be either standard CHAT tier names (`%mor`, `%gra`, `%wor`, `%tim`, `%pho`, `%mod`, `%spa`, `%add`, `%act`, `%sit`, `%com`, `%int`, `%ort`, `%err`, `%exp`, `%gpx`, `%trn`, `%coh`, `%def`, `%fac`, `%par`, `%eng`) or user-defined tiers prefixed with `%x` (e.g., `%xfoo`, `%xtra`, `%xcoref`).

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

## Notes

This is a warning-level diagnostic. Unsupported dependent tiers are preserved in the model for roundtrip fidelity. The `DependentTier::Unsupported` variant distinguishes truly unknown tiers from intentional user-defined `%x`-prefixed tiers.
