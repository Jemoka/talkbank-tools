# E525: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E525
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_gaps/nested-bg-same-label.cha`
**Trigger**: See example below
**Expected Error Codes**: E529

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Bg:test
@Comment:	This is inside the first @Bg:test scope
@Bg:test
@Comment:	ERROR: This second @Bg:test should be invalid (nested @Bg with same label)
@Eg:test
@Eg:test
@End
```

## Example 2

**Source**: `error_corpus/validation_gaps/lazy-gem-inside-bg.cha`
**Trigger**: See example below
**Expected Error Codes**: E530

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Bg:activity
@Comment:	We are inside a @Bg/@Eg scope
@G:	playing with blocks
@Comment:	ERROR: @G (lazy gem) should not be allowed inside @Bg/@Eg scope
@Eg:activity
@End
```

## Example 3

**Source**: `error_corpus/E5xx_header_errors/E525_unknown_header.cha`
**Trigger**: See example below
**Expected Error Codes**: E525

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|test|CHI|||||Child|||
@UnknownHeader:	this header does not exist
*CHI:	hello .
@End
```

## Example 4

**Source**: `error_corpus/E5xx_header_errors/E526_unmatched_begin_gem.cha`
**Trigger**: See example below
**Expected Error Codes**: E526

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
@Bg:	episode1
*CHI:	hello world .
@End
```

## Example 5

**Source**: `error_corpus/E5xx_header_errors/E527_unmatched_end_gem.cha`
**Trigger**: See example below
**Expected Error Codes**: E527

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
*CHI:	hello world .
@Eg:	episode1
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
