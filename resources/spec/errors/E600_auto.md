# E600: Tier alignment skipped due to parse errors

## Description

A dependent tier (typically `%mor`) had parse errors during lenient recovery, so the
validator cannot verify alignment between tiers. Alignment checks (main↔%mor, %mor↔%gra)
are skipped for the affected utterance. This is a **warning**, not an error — the file
still parses, but alignment correctness is unverified for tainted tiers.

E600 fires in pairs: if `%mor` is tainted, both main↔%mor and %mor↔%gra alignment
checks are skipped, producing two E600 warnings for the same utterance.

## Metadata
- **Status**: implemented

- **Error Code**: E600
- **Category**: validation
- **Level**: tier
- **Layer**: validation

## Example 1 — Non-integer index in %gra

**Trigger**: `abc|0|ROOT` in `%gra` — a non-numeric index causes the tree-sitter
grammar to reject the token, creating an ERROR node. The error recovery recognizes
this as a `%gra:` tier with parse errors and emits E600.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	int|hello n|world .
%gra:	abc|0|ROOT 2|0|ROOT
@End
```

**Expected**: E600 warning — could not fully parse dependent tier content.

## Root Cause

E600 is a downstream consequence, not a primary error. The actual problem is in the
dependent tier content (e.g. malformed %gra relation). When tree-sitter encounters
a parse error in a `%mor`, `%gra`, `%pho`, or `%sin` tier, the error recovery marks
the tier as tainted and emits E600 as a warning.

## CHAT Rule

Dependent tiers must parse cleanly for alignment validation to run. See CHAT manual
sections on %gra tier format: each relation must be `index|head|RELATION`.

## Notes

- E600 fires when tree-sitter produces an ERROR node inside a %mor, %gra, %pho, or
  %sin tier. The tier content is recognized as belonging to a known tier type, but
  has internal parse errors.
- Fix the underlying tier parse error and E600 goes away.
- Re-running morphotag will regenerate tiers from scratch, eliminating bad content.
