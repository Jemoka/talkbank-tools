# E360: Invalid media bullet

## Description

Media bullet (timestamp marker) contains malformed content — e.g., non-numeric characters, missing underscore separator, or structurally invalid timestamp format.

## Metadata

- **Status**: not_implemented
- **Last updated**: 2026-04-13 14:42 EDT
- **Error Code**: E360
- **Category**: validation
- **Level**: utterance
- **Layer**: parser
- **Status note**: Partially unreachable via tree-sitter parser. E360 is emitted in three code paths: (1) `parse_media_bullet` for legacy token-style bullets (dead code for structured grammar), (2) `parse_internal_bullet` for content-area bullets where `parse_bullet_node_timestamps` returns None, and (3) `parse_inline_bullet` for dependent-tier bullets where `0_0` IS checked. However, the grammar's `bullet` rule (`bullet_timestamp: /[0-9]+/`) pre-validates numeric content, so `parse_bullet_node_timestamps` always succeeds for grammar-accepted bullets. The `0_0` check in `parse_media_bullet` and `parse_inline_bullet` fires, but `utterance_end.rs` and `internal_bullet.rs` skip this check — `0_0` bullets pass through to the model where E362 (start >= end) fires instead.

## Example 1

**Source**: `E3xx_main_tier_errors/E360_invalid_media_bullet.cha`
**Trigger**: Zero-duration timestamp (start == end, both 0ms) — triggers E362 instead
**Expected Error Codes**: E362

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello . 0_0
@End
```

## Expected Behavior

E360 should fire for malformed media bullets (non-numeric timestamps, missing
underscore separator). The grammar pre-validates numeric content, so only
`parse_bullet_text` failures on non-standard text trigger E360. The `0_0` case
triggers E362 (backwards/zero-duration) at the validation layer instead.

## CHAT Rule

See CHAT manual on media bullets. Format: `\u{15}start_end\u{15}` where start
and end are non-negative integers in milliseconds with start < end.

## Notes

- E360 code exists in multiple emission sites but most are unreachable due to
  the grammar pre-validating bullet content as numeric
- `0_0` bullets: `utterance_end.rs` and `internal_bullet.rs` don't check for
  this case, so they create Bullet(0,0) which then fails E362 validation
- Only `parse_inline_bullet` (dependent tier bullets) checks for `0_0`
- Non-numeric bullet content is rejected by the grammar before E360 can fire
