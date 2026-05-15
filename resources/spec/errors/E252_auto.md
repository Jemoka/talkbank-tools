# E252: Syntax error - caret at word start

## Description

Syntax error - caret at word start

## Metadata
- **Status**: not_implemented
- **Layer**: validation

- **Error Code**: E252
- **Category**: Prosodic marker placement
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `E3xx_main_tier_errors/E303_caret_at_word_start.cha`
**Trigger**: Caret (^) used at word start instead of mid-word
**Expected Error Codes**: E252

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Caret for pause between syllables must appear MID-WORD (e.g., rhi^noceros)
@Comment:	not at the start of a word
*CHI:	^test .
@End
```

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
