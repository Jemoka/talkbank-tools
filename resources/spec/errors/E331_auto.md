# E331: UnexpectedNodeInContext

## Description

A tree-sitter node appeared in a syntactic context where it is not expected. The node
type itself is valid CHAT syntax, but it occurs at a position in the AST that violates
the grammar. This error is emitted during tree-sitter error recovery — the parser
attempts to continue after encountering invalid syntax, and the recovered structure
contains nodes in unexpected positions.

## Metadata
- **Status**: not_implemented

- **Error Code**: E331
- **Category**: parser_recovery
- **Level**: utterance
- **Layer**: parser

## Example 1 — Missing stem in %mor word

**Source**: `childes-data/Eng-NA/MacWhinney/070518a.cha` (line 1800)
**Trigger**: `noun|-Acc` in `%mor` — the stem is empty (nothing between `|` and `-`).
Tree-sitter's error recovery produces a `mor_content` node where the "main chunk"
(stem) should be, triggering E331.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	MAR Target_Child
@ID:	eng|corpus|MAR|||||Target_Child|||
*MAR:	don't give me a YM . 1520652_1522904
%mor:	aux|do-Fin-Imp-S~part|not verb|give-Fin-Imp-S pron|I-Prs-Acc-S1 det|a-Ind-Art noun|-Acc .
%gra:	1|3|AUX 2|3|ADVMOD 3|6|ROOT 4|3|IOBJ 5|6|DET 6|3|OBJ 7|3|PUNCT
@End
```

**Expected**: E331 at `noun|-Acc` — unexpected `mor_content` in `mor_content` missing
main chunk. Also triggers E342 (missing stem) and E600 (alignment skipped).

**Error message**: `Unexpected 'mor_content' in mor_content missing main chunk`

## Root Cause

The word "YM" on the main tier has no known morphological analysis. The %mor tier
records `noun|-Acc` — a POS tag and case suffix with no stem. The tree-sitter %mor
grammar expects `pos|stem(-suffix)*`, so the empty stem causes the parser to enter
error recovery. The recovered AST has a `mor_content` node where the stem chunk
should be, which the Rust walker flags as E331.

## Corpus Impact

- 1 file, 1 occurrence
- `childes-data/Eng-NA/MacWhinney/070518a.cha` — `noun|-Acc` for the word "YM"
- Extremely rare; co-occurs with E342 (same token)

## CHAT Rule

Each %mor word must have the format `pos|stem(-suffix)*` where the stem is non-empty.
See CHAT manual section on %mor tier morphological coding.

## Notes

- This is a parser-recovery error — tree-sitter tried to continue after invalid syntax
- The specific error message includes the unexpected node type and the context where
  it appeared (e.g., "mor_content in mor_content missing main chunk")
- Always co-occurs with other error codes on the same token (E342 for missing stem,
  E600 for skipped alignment)
- Re-running morphotag will regenerate %mor from scratch, producing a valid stem
