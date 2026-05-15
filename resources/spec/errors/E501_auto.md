# E501: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E501
- **Category**: Header validation
- **Level**: header
- **Layer**: parser

## Example 1

**Source**: `error_corpus/E5xx_header_errors/E501_duplicate_header.cha`
**Trigger**: Two @Begin headers
**Expected Error Codes**: E501

```chat
@UTF8
@Begin
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
```

## Example 2

**Source**: `error_corpus/E5xx_header_errors/E512_participant_no_role.cha`
**Trigger**: @Participants with only participant code, no role
**Expected Error Codes**: E513

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI
@ID:	eng|corpus|CHI|||||CHI|||
@End
```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on file headers and metadata. Headers like @Participants, @Languages, and @ID have specific format requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
