# E371: Pause inside phonological group

## Description

Pause inside phonological group

## Metadata

- **Error Code**: E371
- **Category**: validation
- **Level**: utterance
- **Layer**: validation

## Example 1

**Source**: `validation_gaps/pause-in-pho-group.cha`
**Trigger**: See example below
**Expected Error Codes**: E371

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	hello ‹hɛ (.) loʊ› there .
@Comment:	ERROR: Pause (.) cannot be embedded inside phonological group ‹...›
*CHI:	goodbye ‹gʊd baɪ› friend .
@Comment:	VALID: No pause inside ‹...›
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
