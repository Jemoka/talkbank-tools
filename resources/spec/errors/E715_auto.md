# E715: `%pho` alignment count mismatch — too many tokens

## Description

The `%pho` (actual phonology) tier has more alignable tokens than the main tier.
Remove the extra `%pho` tokens so counts match.

`%mod` count mismatches use E734. `%wor` is not an alignment tier — it is a
timing sidecar (`WorTimingSidecar`) modeled in
[`talkbank-model::alignment`](../../crates/talkbank-model/src/alignment/wor.rs),
so no E7xx error fires on a `%wor` count mismatch; drift is reported
structurally via the `Drifted` variant, not via `ParseError`.

## Metadata

- **Error Code**: E715
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/E715_pho_count_too_many.cha`
**Trigger**: Main tier has 2 words, but %pho has 3 tokens
**Expected Error Codes**: E715

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	want cookie .
%pho:	aɪ wɑnt kʊki
@Comment:	ERROR: Main tier has 2 words but %pho has 3 tokens (extra aɪ)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- E715 is scoped to `%pho` only; `%mod` uses E734. `%wor` is a timing sidecar, not an alignment — see `WorTimingSidecar`.
