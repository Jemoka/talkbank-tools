# INDENT -- Align CA Overlap Markers

**Status:** Current
## Purpose

Aligns overlap markers in Conversation Analysis (CA) transcripts. The legacy manual describes `INDENT` simply as a program for realigning overlap marks in CA files, and notes that the files must use a fixed-width font such as CAFont.

`clan-core` aligns closing overlap markers (`⌊`, U+230A) by column position with their matching opening overlap markers (`⌈`, U+2308) on a preceding speaker tier.

## Usage

```bash
chatter clan indent file.cha
chatter clan indent file.cha -o aligned.cha
```

## Algorithm

1. Parse the file into tiers (speaker prefix + content text)
2. For each main tier (`*SPK:`), scan for opening overlap markers `⌈` and record their column positions and optional numeric suffixes
3. Scan up to 30 subsequent tiers from *different* speakers for closing overlap markers `⌊`
4. Match open/close pairs by numeric suffix (or sequentially if unnumbered)
5. Insert or remove spaces before the closing marker to align columns
6. Report unmatched markers as warnings

## Example

Before:
```text
*CHI:	I want ⌈ cookies ⌉ .
*MOT:	⌊ yeah ⌋ okay .
```

After:
```text
*CHI:	I want ⌈ cookies ⌉ .
*MOT:	       ⌊ yeah ⌋ okay .
```

Numbered overlaps (`⌈1`, `⌈2`, etc.) are matched by their numeric suffix, allowing multiple simultaneous overlaps to be aligned independently.

## Differences from CLAN

- **Manual intent**: `INDENT` is a layout command, not a semantic CHAT analysis command.
- Operates on UTF-8 text using Rust's `char`-based column counting rather than C byte-level scanning.
- Uses the text-based transform pattern (no AST round-trip) to preserve original formatting outside of overlap alignment.
- Maximum 10 alignment passes (CLAN's `goto beginAgain` loop has no bound, causing infinite loops on some inputs).
- Column counting treats each Unicode scalar value as width 1.
