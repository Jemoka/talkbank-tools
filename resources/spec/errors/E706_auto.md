# E706: Mor count mismatch - too many mor items

## Description

Mor count mismatch - too many mor items

## Metadata

- **Error Code**: E706
- **Category**: Alignment count mismatch
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/E706_mor_count_too_many.cha`
**Trigger**: Main tier has 2 words + terminator = 3 alignable items, but %mor has 4
**Expected Error Codes**: E706

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	want cookie .
%mor:	pro|I v|want n|cookie .
@Comment:	ERROR: Main tier has 2 words but %mor has 3 items (extra pro|I)
@End
```

## Example 2

**Source**: `E4xx_alignment_errors/scoped_annotation_alignment.cha`
**Trigger**: Content in scoped annotations [//] [/] should be filtered
**Expected Error Codes**: E706

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Retraces and scoped content don't align
*CHI:	I want [/] need cookie .
%mor:	pro|I v|want v|need n|cookie .
@Comment:	ERROR: "want" is in a retrace [/] scope and shouldn't align
@Comment:	Main tier alignable: I, need, cookie = 3 words (want is excluded by [/])
@Comment:	Mor tier: Should be pro|I v|need n|cookie (3 items + terminator)
@End
```

## Example 3

**Source**: `E4xx_alignment_errors/event_no_alignment.cha`
**Trigger**: Events &=laughs don't get mor items
**Expected Error Codes**: E706

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Events are non-alignable, like pauses
*CHI:	I &=laugh want cookie .
%mor:	pro|I v|laugh v|want n|cookie .
@Comment:	ERROR: Event &=laugh shouldn't have a mor item (v|laugh is wrong)
@Comment:	Main tier alignable: I, want, cookie = 3 words (events excluded)
@Comment:	Mor tier: Should be pro|I v|want n|cookie (3 items + terminator)
@End
```

## Example 4

**Source**: `E4xx_alignment_errors/pause_no_alignment.cha`
**Trigger**: Pauses (.) don't get mor items
**Expected Error Codes**: E706

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Pauses, events, actions don't align - only words
*CHI:	I (.) want cookie .
%mor:	pro|I v|pause v|want n|cookie .
@Comment:	ERROR: Pause (.) shouldn't have a mor item (v|pause is wrong)
@Comment:	Main tier alignable: I, want, cookie = 3 words (pauses excluded)
@Comment:	Mor tier: Should be pro|I v|want n|cookie (3 items + terminator)
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
