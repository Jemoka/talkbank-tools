# Known Differences: BA3 vs BA2 Jan 9 Baseline

**Status:** Draft
**Last updated:** 2026-03-18

This file documents intentional differences between BA3 and the canonical
BA2 Jan 9 baseline (`84ad500b`). Each entry explains why the difference exists
and whether BA3 is correct (fixed a BA2 bug) or deliberately changed.

The `assert_ba2_parity()` test helper automatically filters metadata lines
that naturally differ (`@PID`, `@Date`, `@Tape Location`, etc.).

## Morphotag

### %gra ROOT convention (all languages)

BA2 uses self-referencing ROOT: `4|7|ROOT` (word 4 points to word 7, which is itself the root).
BA3 uses UD-standard ROOT: `4|0|ROOT` (root points to 0, the conventional UD sentinel).

This affects every `%gra` line in every language. BA3's convention is correct per
Universal Dependencies. The difference is systematic and intentional.

**Status:** Intentional BA3 improvement. Parity assertion normalizes ROOT targets.

### mm-hmm tokenization (English)

BA2: `intj|mmhmm` (merged form).
BA3: `intj|mm–hmm` (preserves hyphenation with en-dash).

Stanza version difference in tokenization of hyphenated interjections.

**Status:** Cosmetic. No functional impact.

### @Participants/@ID ordering

BA3 may reorder `@Participants` and `@ID` lines relative to BA2.
Both orderings are valid CHAT.

**Status:** Cosmetic. Parity assertion ignores participant ordering.

## Utseg

_To be populated._

## Translate

_To be populated._

## Coref

BA2 coref is broken in both jan9 and master — Stanza 1.11.0 coref model
(`ontonotes-singletons_roberta-large-lora`) crashes with:
`Config.__init__() missing 1 required positional argument: 'plateau_epochs'`

This is a Stanza version incompatibility, not a BA2 code bug. No BA2 golden
outputs can be generated for coref. BA3 coref tests rely on existing
`golden.rs` snapshot tests (which use the live BA3 Stanza worker).

## Transcribe

### D1: Disfluency replacement (MISSING in BA3)

BA2 runs `DisfluencyReplacementEngine` after ASR, converting raw words
like "um" and "uh" into CHAT disfluency markers `&-um`, `&-uh` using
language-specific wordlists (`support/filled_pauses.eng`).

BA3 does not implement this stage yet. Transcribe output will differ.

**Status:** Expected failure until D1 is implemented.

### D1b: N-gram retrace marking (MISSING in BA3)

BA2 runs `NgramRetraceEngine` after ASR, detecting repeated n-grams
and marking them as `<word word> [/]` retraces.

BA3 does not implement this stage yet.

**Status:** Expected failure until D1b is implemented.

## Align

_To be populated._
