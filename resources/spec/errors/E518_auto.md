# E518: Auto-generated from corpus

## Description

Auto-generated from corpus

## Metadata

- **Error Code**: E518
- **Category**: validation
- **Level**: header
- **Layer**: validation

## Example 1

**Source**: `error_corpus/E5xx_header_errors/E518_date_single_digit_day.cha`
**Trigger**: See example below
**Expected Error Codes**: E518

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	1-JAN-2024
@End

*CHI:	hello .
```

## Example 2

**Source**: `error_corpus/E5xx_header_errors/E518_date_wrong_separator.cha`
**Trigger**: See example below
**Expected Error Codes**: E518

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	01/01/2024
@End

*CHI:	hello .
```

## Example 3

**Source**: `error_corpus/E5xx_header_errors/E518_date_invalid_month.cha`
**Trigger**: See example below
**Expected Error Codes**: E518

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	01-ABC-2024
@End

*CHI:	hello .
```

## Example 4

**Source**: `error_corpus/E5xx_header_errors/E518_date_day_out_of_range.cha`
**Trigger**: See example below
**Expected Error Codes**: E518

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	32-JAN-2024
@End

*CHI:	hello .
```

## Example 5

**Source**: `error_corpus/E5xx_header_errors/E513_invalid_date_format.cha`
**Trigger**: @Date with invalid format (should be DD-MMM-YYYY not full month name)
**Expected Error Codes**: E518

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Date:	01-January-2025
@End
```

## Example 6

**Source**: `error_corpus/E5xx_header_errors/E518_date_lowercase_month.cha`
**Trigger**: See example below
**Expected Error Codes**: E518

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	01-jan-2024
@End

*CHI:	hello .
```

## Expected Behavior

The validator should detect the malformed date format and report error E518.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on file headers and metadata. Headers like @Participants, @Languages, and @ID have specific format requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
