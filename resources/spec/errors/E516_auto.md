# E516: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E516
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E516_empty_date.cha`
**Trigger**: See example below
**Expected Error Codes**: E516

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	
@Comment:	ERROR: Date header cannot be empty
*CHI:	hello .
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
