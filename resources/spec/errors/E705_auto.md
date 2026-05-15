# E705: Mor count mismatch - too few items

## Description

Mor count mismatch - too few items

## Metadata

- **Error Code**: E705
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/tag_marker_alignment.cha`
**Trigger**: Tag markers (±) should have corresponding mor items
**Expected Error Codes**: E705

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Tag markers are alignable content
*CHI:	I want ± cookie .
%mor:	pro|I v|want n|cookie .
@Comment:	ERROR: Tag marker ± should have a mor item
@Comment:	Main tier alignable: I, want, ±, cookie = 4 words
@Comment:	Mor tier: Should have 4 items (missing item for ±)
@End
```

## Example 2

**Source**: `E4xx_alignment_errors/E705_mor_count_too_few.cha`
**Trigger**: Main tier has 3 words + terminator = 4 alignable items, but %mor has only 2
**Expected Error Codes**: E705

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want .
@Comment:	ERROR: Main tier has 3 words but %mor only has 2 items (missing n|cookie)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
