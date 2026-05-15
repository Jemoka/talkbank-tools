# E342: Missing required element

## Description

Missing required element

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E342
- **Category**: Word validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E2xx_word_errors/E211_replacement_missing_corrected.cha`
**Trigger**: Replacement containing 0 (omission marker)
**Expected Error Codes**: E390

Note: The replacement `[: 0]` contains an omission marker (`0`), which
triggers E390 (ReplacementContainsOmission) rather than E316 (UnparsableContent)
or E342 (MissingRequiredElement). The parser successfully parses the replacement
and then validates its content.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	helo [: 0] world .
@End
```

## Example 2

**Source**: `E7xx_tier_parsing/E704_empty_mor_pos.cha`
**Trigger**: %mor chunk with empty part-of-speech before pipe
**Expected Error Codes**: E316, E702

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	|hello n|world .
@End
```

## Example 3

**Source**: `E7xx_tier_parsing/E703_empty_mor_stem.cha`
**Trigger**: %mor chunk with empty stem after pipe
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	v| n|world .
@End
```

## Example 4

**Source**: `E7xx_tier_parsing/E711_gra_missing_role.cha`
**Trigger**: %gra relation with empty role field
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%gra:	1|2| 2|0|ROOT
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
