# E404: Orphaned dependent tier

**Last updated:** 2026-04-13 23:00 EDT

## Description

A dependent tier (`%mor`, `%gra`, etc.) appears before any main tier in the
file. E404 (OrphanedDependentTier) is emitted by
`report_top_level_dependent_tier_error()` in
`crates/talkbank-parser/src/parser/chat_file_parser/chat_file/helpers.rs`
when a `%`-prefixed ERROR node appears before any utterance.

## Metadata
- **Status**: implemented

- **Error Code**: E404
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E404_orphaned_dependent_tier.cha`
**Trigger**: `%mor:` tier appears before any `*SPK:` main tier, but a
`*SPK:` line follows so that the grammar still recognizes the surrounding
file structure. The orphaned dependent tier is captured as a top-level
ERROR node, which `report_top_level_dependent_tier_error()` classifies
and emits as E404.
**Expected Error Codes**: E404

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
%mor:	co|hello .
*CHI:	hello .
@End
```

## Expected Behavior

The parser reports E404 for the orphaned dependent tier line. The tier
must appear at the top level of the file (before any `*SPK:` main tier)
for this code to fire; once the file has at least one preceding
utterance, a stray dependent tier is attached to the prior utterance
instead.

## CHAT Rule

See CHAT manual sections on dependent tier syntax (%mor, %gra, etc.). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The earlier example in this spec placed `%mor:` immediately after the
  headers with no following main tier; that shape caused tree-sitter's
  error recovery to fail catastrophically and surface as E501-E505
  instead of E404. Keeping a valid `*CHI:` line after the orphaned tier
  lets the grammar recognize the file structure, so the Rust parser's
  top-level ERROR classifier runs and produces E404 as intended.
