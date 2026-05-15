# E733: `%mod` alignment count mismatch — too few tokens

## Description

The `%mod` (model/target phonology) tier has fewer alignable tokens than the
main tier. Each main-tier word must have a corresponding `%mod` token.

This code is scoped to `%mod` only. `%pho` count mismatches use E714. `%wor`
is a timing-annotation tier and is never validated for count mismatches.

## Metadata

- **Error Code**: E733
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/E733_mod_count_too_few.cha`
**Trigger**: Main tier has 3 words, but %mod has only 2 tokens
**Expected Error Codes**: E733

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mod:	aɪ wɑnt
@Comment:	ERROR: Main tier has 3 words but %mod only has 2 tokens (missing cookie)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Introduced when %pho/%mod/%wor alignment codes were separated (previously all used E714/E715)
- E733 is scoped to `%mod` only; `%pho` uses E714
