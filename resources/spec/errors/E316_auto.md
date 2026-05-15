# E316: Unparsable content

## Description

Unparsable content

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E316
- **Category**: Main tier validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E3xx_main_tier_errors/E309_speaker_in_same_bullet.cha`
**Trigger**: Same speaker appears twice in bullet group
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello . [+ bch] 2041689_2042652
*CHI:	world . [+ bch] 2051689_2052652
@End
```

## Example 2

**Source**: `E3xx_main_tier_errors/E331_unexpected_node_helper.cha`
**Trigger**: Try to trigger internal parser bug with unexpected parse node
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	hello {{{ world }}} .
@End
```

## Example 3

**Source**: `E3xx_main_tier_errors/E330_unexpected_node_content.cha`
**Trigger**: Try to trigger internal parser bug with unusual content
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	<<< [= test] >>> .
@End
```

## Example 4

**Source**: `E3xx_main_tier_errors/E330_unusual_content_marker.cha`
**Trigger**: Try to trigger internal parser bug in structure parsing
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	<<<<< hello >>>>> world .
@End
```

## Example 5

**Source**: `E3xx_main_tier_errors/E303_syntax_error.cha`
**Trigger**: Malformed main tier syntax
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI	hello world .
@End
```

## Example 10

**Source**: `E5xx_header_errors/E501_duplicate_header.cha`
**Trigger**: Two @Begin headers
**Expected Error Codes**: E501

Note: The duplicate `@Begin` header is detected as E501 (DuplicateHeader)
rather than E316 (UnparsableContent) because the parser has a specific check
for duplicate headers.

```chat
@UTF8
@Begin
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
```

## Example 13

**Source**: `E5xx_header_errors/E515_bullet_time_invalid.cha`
**Trigger**: Within bullet, start_ms >= end_ms (should be start < end)
**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: Timestamp shows 2052652_2041689 where start > end
*CHI:	hello world . [+ bch] 2052652_2041689
@End
```

## Example 17

**Source**: `E7xx_tier_parsing/E702_invalid_mor_format.cha`
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

## Expected Behavior

The parser should successfully parse these CHAT files (unless marked as parser layer), and the appropriate error should be reported.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
