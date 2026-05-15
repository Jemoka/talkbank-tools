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
