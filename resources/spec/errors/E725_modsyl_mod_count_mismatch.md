# E725: Modsyl tier word count does not match mod tier

## Description

The `%xmodsyl` tier word count does not match the `%mod` tier word count. Each word-level entry in `%xmodsyl` must correspond one-to-one with a word-level entry in `%mod`.

## Metadata

- **Error Code**: E725
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E725

```chat
@UTF8
@Begin
@Languages:	nld
@Participants:	CHI Child, MOT Mother
@ID:	nld|corpus|CHI|2;00.||||Child|||
@ID:	nld|corpus|MOT|||||Mother|||
*MOT:	muts ja .
%mod:	ˈmœts ˈja
%xmodsyl:	ˈm:Oœ:Nt:Cs:R
@Comment:	ERROR: %mod has 2 words but %xmodsyl has only 1
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E725 — `%xmodsyl` word count (1) does not match `%mod` word count (2)

## CHAT Rule

The `%xmodsyl` tier provides syllable-level morphological annotation and must have the same number of word-level entries as the corresponding `%mod` tier.

## Notes

Validated in `alignment.rs` via `build_tier_to_tier_alignment()` which compares word counts between `%xmodsyl` and `%mod` tiers within the same utterance.
