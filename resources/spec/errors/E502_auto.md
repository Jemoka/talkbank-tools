# E502: Missing required @End header

## Description

Every valid CHAT file must end with an `@End` header. This error indicates the file is missing `@End`, usually because the file is truncated, empty, or was saved incompletely.

## Metadata

- **Error Code**: E502
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Corpus Impact

| Collection | Files |
|------------|------:|
| aphasia-data | 154 |
| dementia-data | 85 |
| ca-data | 10 |
| rhd-data | 5 |
| tbi-data | 4 |
| slabank-data | 3 |
| **Total** | **160** (unique files) |

## Example 1: Truncated file

**Source**: `error_corpus/validation_errors/E502_missing_end_header.cha`
**Trigger**: File ends without `@End`
**Expected Error Codes**: E502

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
```

## Example 2: Corpus — Kurland PWA (aphasia-data)

**Trigger**: Many files in aphasia-data Kurland/PWA corpus lack `@End`
**Corpus**: aphasia-data/English/Protocol/Kurland/PWA

These files typically have all headers and content but simply lack the final `@End` line. They were likely saved by an older version of CLAN that did not enforce `@End`.

## Expected Behavior

The parser should report E502 pointing at the **end of the file** (not the beginning). The error location should help the user find where `@End` should be added.

**Note**: Prior to the fix in this branch, E502 was reported at `(line 1, column 1, bytes 0..0)` — pointing at the beginning of the file. This was misleading since the problem is at the end. The fix changes the error location to point at the last byte of the file.

## CHAT Rule

Every CHAT file must begin with `@Begin` and end with `@End`. See CHAT manual section on file structure: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- All 160 affected files are pre-existing data quality issues, not parser bugs
- Most are in aphasia-data (154 files) and dementia-data (85 files)
- Fix: add `@End` at the end of each affected file
- These files may also have other structural issues (missing headers, truncated content)

## False positive: %wor parse error cascades to entire file

A separate parser bug can cause E502 to fire on files that **do** have `@End`.
When a `%wor` tier contains invalid content (e.g., an action marker like
`&=head:no`) AND the `%wor` line has 7+ words after the error, tree-sitter's
error recovery fails catastrophically: instead of isolating the ERROR to the
`%wor` tier, the entire file becomes one ERROR node. Effects:

1. `@End` is not recognized as a header.
2. Validation falsely reports E502 "Missing required @End header".
3. All other validation is also lost.

This is NOT a missing `@End` — affected files **do** have `@End`. It is a
tree-sitter error recovery cascade triggered by long invalid `%wor` content.

### Minimal reproduction

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

**Expected**: localized ERROR on the `%wor` tier; `@End` recognized; no E502.
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

### Root cause

Tree-sitter's error recovery uses a cost heuristic. When the invalid region in
`wor_tier_body` is short (few tokens), tree-sitter can recover by skipping the
bad tokens and continuing to parse subsequent lines. When the invalid region is
long (7+ words with timing bullets = 14+ tokens), the error cost exceeds
tree-sitter's threshold and it abandons the current `chat_file` production
entirely, wrapping everything in a single ERROR node.

### Possible fixes

1. **Grammar**: add an explicit `ERROR` recovery rule in `wor_tier_body` that
   consumes to end-of-line, preventing the error from propagating past the tier
   boundary.
2. **Grammar**: increase tree-sitter's error cost tolerance (if configurable).
3. **Rust parser**: when the tree-sitter parse produces a file-level ERROR,
   fall back to line-by-line header scanning to at least recognize `@End`.

### Notes

- This bug exists in the current grammar — it is NOT a new regression.
- The `%wor` content (`&=head:no`, `&=ges:fall`, etc.) is pre-existing legacy CLAN data.
- Once these files are re-aligned with the Rust backend, the bad `%wor` content
  will be replaced and E502 will no longer fire.
