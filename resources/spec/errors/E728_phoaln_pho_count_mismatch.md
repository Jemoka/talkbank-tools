# E728: Phoaln tier word count does not match pho tier

## Description

The `%xphoaln` tier word count does not match the `%pho` tier word count. Each word-level entry in `%xphoaln` must correspond one-to-one with a word-level entry in `%pho`.

## Metadata

- **Error Code**: E728
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E728

```chat
@UTF8
@Begin
@Languages:	nld
@Participants:	CHI Child, MOT Mother
@ID:	nld|corpus|CHI|2;00.||||Child|||
@ID:	nld|corpus|MOT|||||Mother|||
*MOT:	muts ja .
%pho:	ˈmʉst ˈjɛ
%xphoaln:	m↔m,œ↔ʉ,t↔s,s↔t
@Comment:	ERROR: %pho has 2 words but %xphoaln has only 1
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E728 — `%xphoaln` word count (1) does not match `%pho` word count (2)

## CHAT Rule

The `%xphoaln` tier provides phonological alignment annotation and must have the same number of word-level entries as the corresponding `%pho` tier.

## Notes

Validated in `alignment.rs` via `build_count_mismatch_error()` which compares word counts between `%xphoaln` and `%pho` tiers within the same utterance.
