# W601: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: W601
- **Category**: Warnings
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `W6xx_warnings/W601_empty_tier.cha`
**Trigger**: Dependent tier with no content
**Expected Error Codes**: E342

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Expected tree-sitter warning: E342 (Empty dependent tier content)
@Comment:	Expected direct warning: W601
*CHI:	hello world .
%xfoo:	
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
