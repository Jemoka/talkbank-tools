# E727: Phoaln tier word count does not match mod tier

## Description

The `%xphoaln` tier word count does not match the `%mod` tier word count. Each word-level entry in `%xphoaln` must correspond one-to-one with a word-level entry in `%mod`.

## Metadata

- **Error Code**: E727
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E727

```chat
@UTF8
@Begin
@Languages:	nld
@Participants:	CHI Child, MOT Mother
@ID:	nld|corpus|CHI|2;00.||||Child|||
@ID:	nld|corpus|MOT|||||Mother|||
*MOT:	muts ja .
%mod:	ˈmœts ˈja
%xphoaln:	m↔m,œ↔ʉ,t↔s,s↔t
@Comment:	ERROR: %mod has 2 words but %xphoaln has only 1
@End
```

## Expected Behavior

- **Parser**: Should succeed — syntax is valid
- **Validator**: Should report E727 — `%xphoaln` word count (1) does not match `%mod` word count (2)

## CHAT Rule

The `%xphoaln` tier provides phonological alignment annotation and must have the same number of word-level entries as the corresponding `%mod` tier.

## Notes

Validated in `alignment.rs` via `build_count_mismatch_error()` which compares word counts between `%xphoaln` and `%mod` tiers within the same utterance.
