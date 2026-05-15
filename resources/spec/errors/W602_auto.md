# W602: Deprecated experimental tier name

## Description

A user-defined dependent tier (`%x...`) uses a label that matches a known
standard tier name. For example, `%xpho` should be updated to `%pho` since
`pho` is now a recognized standard tier. This is a warning to encourage
migration from legacy experimental naming to the current standard.

**Validation not yet implemented for this spec example.** The check in
`crates/talkbank-model/src/validation/unparsed_tier.rs` fires when
`check_user_defined_tier_content()` is called with a label matching a known
standard tier. However, the parser may handle `%xpho:` differently — it might
parse `%xpho` as a recognized tier type rather than routing it through the
user-defined tier validation path. The `UserDefined` variant of `DependentTier`
stores the label without the `x` prefix; the check compares against
`KNOWN_TIERS` which includes `"pho"`.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT
- **Layer**: validation

- **Error Code**: W602
- **Category**: Warnings
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `W6xx_warnings/W602_deprecated_xtier.cha`
**Trigger**: %xpho should be updated to %pho
**Expected Error Codes**: W602

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%xpho:	hɛloʊ wɜɹld
@End
```

## Expected Behavior

The validator should warn (W602) when a `%x`-prefixed tier uses a label that
is now a standard tier name. The check exists in
`crates/talkbank-model/src/validation/unparsed_tier.rs`.

**Trigger conditions**: A `DependentTier::UserDefined` tier whose label
(without the `x` prefix) matches a known standard tier name (e.g., `pho`,
`mor`, `gra`, `sin`, `wor`, `tim`).

## CHAT Rule

See CHAT manual on dependent tiers. The `%x` prefix was historically used for
experimental tiers; many have since been standardized. The CHAT manual is
available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Validation logic exists in `unparsed_tier.rs` with unit tests confirming
  it works at the function level
- The parser may not route `%xpho` through the user-defined tier path, which
  would prevent the W602 check from being reached
- The code IS emitted at the model validation level; the gap is likely in how
  the parser classifies `%xpho` tiers
