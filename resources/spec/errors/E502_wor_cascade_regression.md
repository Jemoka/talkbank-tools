# E502 false positive: %wor parse error cascades to entire file

## Description

When a `%wor` tier contains invalid content (e.g., an action marker like `&=head:no`)
AND the %wor line has 7+ words after the error, tree-sitter's error recovery fails
catastrophically: instead of isolating the ERROR to the %wor tier, the entire file
becomes one ERROR node. This causes:

1. `@End` is not recognized as a header
2. Validation falsely reports E502 "Missing required @End header"
3. All other validation is also lost

This is NOT a missing `@End` — all 160 affected files have `@End`. It is a tree-sitter
error recovery cascade triggered by long invalid %wor content.

## Metadata

- **Error Code**: E502 (false positive)
- **Category**: parser
- **Level**: file
- **Layer**: parser
- **Root Cause**: tree-sitter error recovery threshold exceeded by long invalid %wor

## Minimal Reproduction

7 words after the action marker triggers the cascade. 6 words does not.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	PAR Participant
@ID:	eng|corpus|PAR|||||Participant|||
*PAR:	a w1 w2 w3 w4 w5 w6 w7 . 100_900
%wor:	a &=head:no 50_100 w1 100_200 w2 200_300 w3 300_400 w4 400_500 w5 500_600 w6 600_700 w7 700_800 .
@End
```

**Expected**: localized ERROR on the %wor tier; `@End` recognized; no E502.
**Actual**: `(ERROR [0, 0] - [EOF])` — entire file is one ERROR node; E502 falsely reported.

### Control: 6 words (no cascade)

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	PAR Participant
@ID:	eng|corpus|PAR|||||Participant|||
*PAR:	a w1 w2 w3 w4 w5 w6 . 100_800
%wor:	a &=head:no 50_100 w1 100_200 w2 200_300 w3 300_400 w4 400_500 w5 500_600 w6 600_700 .
@End
```

**Result**: localized ERROR at the action marker only; `@End` recognized; no E502.

## Corpus Impact

- 160 files across aphasia-data (154), dementia-data (85), ca-data (10), and others
- All have `@End` — every E502 in the corpus is a false positive from this bug

## Root Cause Analysis

Tree-sitter's error recovery uses a cost heuristic. When the invalid region in `wor_tier_body`
is short (few tokens), tree-sitter can recover by skipping the bad tokens and continuing
to parse subsequent lines. When the invalid region is long (7+ words with timing bullets =
14+ tokens), the error cost exceeds tree-sitter's threshold and it abandons the current
`chat_file` production entirely, wrapping everything in a single ERROR node.

## Possible Fixes

1. **Grammar**: Add an explicit `ERROR` recovery rule in `wor_tier_body` that consumes
   to end-of-line, preventing the error from propagating past the tier boundary
2. **Grammar**: Increase tree-sitter's error cost tolerance (if configurable)
3. **Rust parser**: When the tree-sitter parse produces a file-level ERROR, fall back
   to line-by-line header scanning to at least recognize `@End`

## Notes

- This bug exists in the current grammar — it is NOT a new regression
- The %wor content (`&=head:no`, `&=ges:fall`, etc.) is pre-existing legacy CLAN data
- Once these files are re-aligned with the Rust backend, the bad %wor content will be
  replaced and E502 will no longer fire
