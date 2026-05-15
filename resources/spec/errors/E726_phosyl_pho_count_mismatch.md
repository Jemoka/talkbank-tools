# E726: Phosyl tier word count does not match pho tier

## Description

The `%xphosyl` tier word count does not match the `%pho` tier word count. Each word-level entry in `%xphosyl` must correspond one-to-one with a word-level entry in `%pho`.

## Metadata

- **Error Code**: E726
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E726

```chat
@UTF8
@Begin
@Languages:	nld
@Participants:	CHI Child, MOT Mother
@ID:	nld|corpus|CHI|2;00.||||Child|||
@ID:	nld|corpus|MOT|||||Mother|||
*MOT:	muts ja .
%pho:	ˈmʉst ˈjɛ
%xphosyl:	ˈm:Oʉ:Ns:Ct:C
@Comment:	ERROR: %pho has 2 words but %xphosyl has only 1
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E726 — `%xphosyl` word count (1) does not match `%pho` word count (2)

## CHAT Rule

The `%xphosyl` tier provides syllable-level phonological annotation and must have the same number of word-level entries as the corresponding `%pho` tier.

## Notes

Validated in `alignment.rs` via `build_tier_to_tier_alignment()` which compares word counts between `%xphosyl` and `%pho` tiers within the same utterance.
