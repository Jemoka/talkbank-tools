# E704: Speaker self-overlap — overlapping overlap markers

## Description

A single speaker has consecutive utterances with overlap markers (`⌈⌉`/`⌊⌋`) that overlap with each other. Overlap markers should indicate simultaneous speech between **different** speakers, not self-overlap.

## Metadata
- **Status**: implemented

- **Error Code**: E704
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Corpus Impact

~90 occurrences across corpus data (2026-03-19 audit: childes-data 51, samtale-data 36,
dementia-data 3). Concentrated in CA-heavy corpora with dense overlap transcription.
Some are legitimate multi-party overlaps (speaker participates on both sides) or
same-speaker layered annotation. See `docs/overlap-validation-audit.md` in talkbank-dev.

## Example 1: Basic self-overlap

**Source**: `error_corpus/validation_errors/E704_speaker_self_overlap.cha`
**Trigger**: Same speaker has adjacent utterances with conflicting overlap brackets
**Expected Error Codes**: E704

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello ⌈ world ⌉ .
*CHI:	⌊ testing ⌋ .
@End
```

## Corpus Evidence: Spanish CA data

**Trigger**: Same speaker with overlapping timing in consecutive utterances
**Corpus**: slabank-data, ca-data

```text
*M2:	⌊ hhh hhh ⌋ no pude ir al mall ...
*S2:	⌊ eight thirty ⌋ .
```

When the same speaker code appears in both utterances with overlapping brackets, the validator reports E704 because overlap is a between-speaker phenomenon.

## Expected Behavior

The parser should successfully parse the file. Validation should report E704 as an error, indicating that a speaker cannot overlap with themselves.

## CHAT Rule

Overlap markers (`⌈⌉` top, `⌊⌋` bottom) indicate simultaneous speech between two different speakers. A speaker cannot overlap with themselves. See CHAT manual section on overlap notation: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- These are pre-existing data quality issues in the transcription
- Common in CA corpora where transcribers use overlaps extensively
- The validator uses a 500ms temporal tolerance when checking overlap conflicts (E704 is CLAN Error 133)
