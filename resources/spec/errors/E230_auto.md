# E230: Unbalanced CA delimiter

## Description

Compound delimiter (`∆`) is not properly balanced — opening delimiter has no matching closing delimiter.

## Metadata
- **Status**: implemented
- **Layer**: validation

- **Error Code**: E230
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E230_unbalanced_ca_delimiter.cha`
**Trigger**: See example below
**Expected Error Codes**: E230

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Compound delimiter (∆) must be balanced
@Comment:	Invalid: 'hello∆world' - Missing closing ∆
*CHI:	hello∆world .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
