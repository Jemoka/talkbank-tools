# E506: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E506
- **Category**: Header validation
- **Level**: header
- **Layer**: parser

## Example 1

**Source**: `error_corpus/E5xx_header_errors/E506_empty_participants.cha`
**Trigger**: @Participants with empty content after colon-tab
**Expected Error Codes**: E342

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	

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
