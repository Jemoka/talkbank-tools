# E382: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E382
- **Category**: Dependent tier parsing
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `error_corpus/E7xx_tier_parsing/E704_empty_mor_pos.cha`
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

## Example 2

**Source**: `error_corpus/E7xx_tier_parsing/E702_invalid_mor_format.cha`
**Trigger**: %mor chunk without pipe separator
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	hello n|world .
@End
```

## Example 3

**Source**: `error_corpus/E7xx_tier_parsing/E703_empty_mor_stem.cha`
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

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or \!). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
