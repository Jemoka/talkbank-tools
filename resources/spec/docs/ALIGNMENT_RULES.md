# Dependent Tier Alignment Rules

**Last modified:** 2026-03-30 17:12 EDT

This document specifies the alignment validation rules for all dependent tiers
that participate in alignment during CHAT file validation.

## Overview

Six dependent tiers participate in alignment:

| Tier   | Alignment Direction   | Alignment Type  | Domain            |
|--------|-----------------------|-----------------|-------------------|
| `%mor` | Main tier -> `%mor`   | `MorAlignment`  | `AlignmentDomain::Mor` |
| `%gra` | `%mor` -> `%gra`      | `GraAlignment`  | (special)          |
| `%pho` | Main tier -> `%pho`   | `PhoAlignment`  | `AlignmentDomain::Pho` |
| `%mod` | Main tier -> `%mod`   | `PhoAlignment`  | `AlignmentDomain::Pho` |
| `%wor` | Main tier -> `%wor`   | `WorAlignment`  | `AlignmentDomain::Wor` |
| `%sin` | Main tier -> `%sin`   | `SinAlignment`  | `AlignmentDomain::Sin` |

All alignment is **strictly 1:1**. The system counts "alignable content units"
on each side and validates that the counts match exactly.

Tiers that do **not** participate in alignment include: `%act`, `%com`, `%exp`,
`%gpx`, `%int`, `%sit`, `%spa`, `%alt`, `%coh`, `%def`, `%eng`, `%err`,
`%fac`, `%flo`, `%gls`, `%ort`, `%par`, `%tim`, and user-defined (`%x...`)
tiers.

---

## Content Type Alignability Matrix

This table summarizes which main-tier content types count as alignable for each
domain:

| Content Type                           | Mor | Pho | Sin | Wor |
|----------------------------------------|-----|-----|-----|-----|
| Regular words                          | Yes | Yes | Yes | Yes |
| Fillers (`&-um`)                       | No  | Yes | Yes | Yes |
| Nonwords (`&~gaga`)                    | No  | Yes | Yes | No  |
| Fragments (`&+fr`)                     | No  | Yes | Yes | No  |
| Untranscribed (`xxx`, `yyy`, `www`)    | No  | Yes | Yes | No  |
| Omissions (`0word`)                    | No  | No  | No  | No  |
| Tag separators (`,` `"` `+`)          | Yes | No  | No  | No  |
| Pauses (`(.)`, `(..)`, `(...)`)        | No  | Yes | No  | No  |
| Retraced/reformulated words            | No  | Yes | Yes | Yes |
| Retraced/reformulated groups           | No  | Yes | Yes | Yes |
| Replacement words (`[: ...]`)          | *R* | *O* | *O* | *R* |
| PhoGroup (`(^ ... ^)`)                | *I* | 1   | 0   | *I* |
| SinGroup (`{^ ... ^}`)                | *I* | 0   | 1   | *I* |
| Quotation (`+" ...`)                   | *I* | *I* | *I* | *I* |
| AnnotatedAction                        | No  | No  | Yes | No  |
| Timestamp tokens (`100_200`)           | Yes | Yes | Yes | No  |

Legend:
- *R* = counts the **replacement** words (not the original)
- *O* = counts the **original** word as 1 (not the replacements)
- *I* = recurses into the group's inner words
- 1 = counts the entire group as a single alignable unit
- 0 = group is invisible (contributes 0 units)

---

## Per-Tier Rules

### 1. Main tier -> `%mor` (Morphological Tier)

**Rule:** Each alignable unit in the main tier corresponds to exactly one `%mor`
item.

**What counts as alignable:**

- Regular words (excluding fragments, fillers, nonwords, untranscribed)
- Tag marker separators:
  - Comma (`,`) -> `cm|cm` in `%mor`
  - Tag (`"`) -> `end|end` in `%mor`
  - Vocative (`+`) -> `beg|beg` in `%mor`
- Replacement words: when a word has `[: replacement]`, the **replacement**
  words are counted (not the original)
- Groups, PhoGroups, SinGroups, Quotations: their inner words are counted
  recursively

**What is excluded:**

- Retraced/reformulated content (annotations: `[/]`, `[//]`, `[///]`, `[//?]`,
  `[/-]`, `[e]`)
- Fragment-like words: `Nonword`, `Filler`, `PhonologicalFragment` categories
- Untranscribed material: `xxx`, `yyy`, `www` (matched **case-insensitively** —
  uppercase `XXX` is illegal per E241 but still excluded from alignment to
  prevent spurious %mor entries when morphotag runs on legacy corpora)
- Pauses, events, actions

**Terminator handling:** The main tier terminator and the `%mor` terminator are
validated separately. E707 fires if one has a terminator and the other does not.
The terminator is **not** counted in the item count.

**Error codes:**

| Code | Name                    | Meaning                                    |
|------|-------------------------|--------------------------------------------|
| E705 | `MorCountMismatchTooFew`  | `%mor` has fewer items than main tier      |
| E706 | `MorCountMismatchTooMany` | `%mor` has more items than main tier       |
| E707 | (inline)                | Terminator mismatch between main and `%mor`|

**Source:** `alignment/mor.rs`

---

### 2. `%mor` -> `%gra` (Grammatical Relations Tier)

**Rule:** Each `%mor` **chunk** corresponds to exactly one `%gra` relation.

**Critical distinction: chunks vs items.** A single `%mor` item can produce
multiple chunks due to clitics:

- Pre-clitics: `pro$v|be&PRES` = 2 chunks (pre-clitic + main word)
- Post-clitics: `v|be~pro|it` = 2 chunks (main word + post-clitic)
- Formula: `chunk_count = pre_clitics.len() + 1 + post_clitics.len()` per item
- The terminator also counts as a chunk if present

When counts match, the validator also checks that:

- Each `%gra` word index is in range `1..=chunk_count`
- Each `%gra` head index is in range `0..=chunk_count` (0 = ROOT)

**Prerequisite:** `%gra` requires `%mor` to be present. If `%gra` exists
without `%mor`, error E604 fires.

**Error codes:**

| Code | Name                   | Meaning                                               |
|------|------------------------|-------------------------------------------------------|
| E604 | `GraWithoutMor`        | `%gra` present without `%mor`                         |
| E712 | `GraInvalidWordIndex`  | `%mor` has more chunks than `%gra` relations, or word index out of bounds |
| E713 | `GraInvalidHeadIndex`  | `%gra` has more relations than `%mor` chunks, or head index out of bounds |
| E720 | `MorGraCountMismatch`  | General `%mor`/`%gra` count mismatch                  |

**Source:** `alignment/gra/align.rs`

---

### 3. Main tier -> `%pho` (Phonological Transcription Tier)

**Rule:** Each alignable unit in the main tier (Pho domain) corresponds to
exactly one `%pho` token.

**What counts as alignable:**

- **All words** including fragments, untranscribed, fillers, nonwords (they were
  all phonologically produced)
- Pauses (counted as 1 alignable unit)
- PhoGroups (`(^ ... ^)`): counted as **1 single unit** (the entire phonological
  group is one alignment slot)
- Replacement words: the **original** word counts as 1 (not the replacement
  words), unless it is a fragment-with-replacement (which yields 0)
- Retraced/reformulated content: **included** (the words were spoken)

**What is excluded:**

- Omissions (`0word`)
- SinGroups (`{^ ... ^}`) -- counted as 0 for Pho
- Tag separators
- AnnotatedActions

**Error codes:**

| Code | Name                      | Meaning                                   |
|------|---------------------------|-------------------------------------------|
| E714 | `PhoCountMismatchTooFew`  | `%pho` has fewer tokens than main tier    |
| E715 | `PhoCountMismatchTooMany` | `%pho` has more tokens than main tier     |

**Source:** `alignment/pho.rs`

---

### 4. Main tier -> `%mod` (Model/Target Phonological Tier)

**Rule:** `%mod` uses **identical alignment rules** to `%pho`. It reuses the
`PhoAlignment` type and the `AlignmentDomain::Pho` domain.

`%mod` represents the target/model pronunciation (what the speaker intended to
say), while `%pho` represents what was actually produced. The alignment
constraints are the same: one entry per phonologically-present main-tier item.

**Error codes:** Same as `%pho` (E714, E715).

**Source:** `alignment/pho.rs` (reused), orchestrated in
`utterance/metadata/alignment.rs`

---

### 5. Main tier -> `%wor` (Word-Level Timing Tier)

**Rule:** Each alignable unit in the main tier (Wor domain) corresponds to
exactly one `%wor` word.

**What counts as alignable:**

- Regular words
- Fillers (`&-um`) -- they appear in `%wor` tiers as spoken content
- Phonological fragments (`&+...`)
- Nonwords (`&~...`)
- Untranscribed placeholders (`xxx`/`yyy`/`www`)
- Retraced/reformulated content -- retrace does not change `%wor` membership;
  spoken tokens count both inside and outside retrace
- Replacement words: the **original spoken** word slot is counted, not the
  editorial replacement
- PhoGroups and SinGroups: their inner words are counted recursively

**What is excluded:**

- Timestamp tokens (shaped like `100_200`) -- these are `%wor` alignment
  metadata (onset/offset times), not lexical tokens
- Omissions (`0word`)
- Pauses (only words get timing, not pauses)
- Tag separators
- AnnotatedActions
- Overlap markers do not affect `%wor` membership

**Strictness invariant:** the list above is a deterministic membership rule, not
an optionality rule. Once a main-tier item counts for `%wor`, it must align to
exactly one `%wor` word. Omitted fillers are therefore invalid: if `&-mm`
counts on the main tier and `%wor` omits it, E714 is correct.

**Concrete rule consequences:**

- `what's is dis [: this] ?` -> `%wor` aligns `dis`, not `this`
- `xxx snack .` -> `%wor` aligns `xxx snack`
- `&~um a boat .` -> `%wor` aligns `um a boat`
- `<one &+ss> [/] one play ground .` -> `%wor` aligns the fragment in the same
  way it would outside retrace

**Key difference from `%mor`:** `%wor` **includes** retraced words because they
were spoken and need word-level timing. `%mor` **excludes** retraced words
because morphological analysis applies to the intended utterance. `%wor` also
includes spoken fragments, nonwords, and untranscribed placeholders, while
`%mor` excludes them. `%wor` uses the **original spoken** word for replacements;
`%mor` uses the **replacement** word.

**Key difference from `%pho`:** `%wor` **excludes** pauses (pauses don't get
word-level timing). `%pho` also includes non-word produced material like pauses,
whereas `%wor` stays word-slot only. `%wor` and `%pho` both follow the original
spoken word for replacements.

**Error codes:** Currently reuses E714/E715 (`PhoCountMismatchTooFew` /
`PhoCountMismatchTooMany`). The error messages mention "`%wor` tier"
specifically.

| Code | Name                      | Meaning                                   |
|------|---------------------------|-------------------------------------------|
| E714 | `PhoCountMismatchTooFew`  | `%wor` has fewer tokens than main tier    |
| E715 | `PhoCountMismatchTooMany` | `%wor` has more tokens than main tier     |

**Source:** `alignment/wor.rs`

---

### 6. Main tier -> `%sin` (Sign/Gesture Tier)

**Rule:** Each alignable unit in the main tier (Sin domain) corresponds to
exactly one `%sin` gesture/sign token.

**What counts as alignable:**

- **All words** including fragments, untranscribed (same breadth as Pho)
- SinGroups (`{^ ... ^}`): counted as **1 single unit** (the entire sign group
  is one alignment slot)
- AnnotatedActions: counted as 1 alignable unit
- Retraced/reformulated content: **included** (the gestures were produced)

**What is excluded:**

- Omissions (`0word`)
- PhoGroups (`(^ ... ^)`) -- counted as 0 for Sin
- Tag separators
- Pauses

**Error codes:**

| Code | Name                      | Meaning                                   |
|------|---------------------------|-------------------------------------------|
| E718 | `SinCountMismatchTooFew`  | `%sin` has fewer tokens than main tier    |
| E719 | `SinCountMismatchTooMany` | `%sin` has more tokens than main tier     |

**Source:** `alignment/sin.rs`

---

## Parse Health Gating

Before performing any alignment check, the system consults `ParseHealth` to
determine whether both tiers were parsed without recovery. If either tier in an
alignment pair is "tainted" (produced through error recovery), the alignment
check is **skipped** and replaced with a warning (E600, severity Warning):

> "Skipped X alignment because Y had parse errors during recovery."

| Alignment Check   | Required Clean Flags         |
|-------------------|------------------------------|
| Main -> `%mor`    | `main_clean && mor_clean`    |
| `%mor` -> `%gra`  | `mor_clean && gra_clean`     |
| Main -> `%pho`    | `main_clean && pho_clean`    |
| Main -> `%mod`    | `main_clean && mod_clean`    |
| Main -> `%wor`    | `main_clean && wor_clean`    |
| Main -> `%sin`    | `main_clean && sin_clean`    |

This prevents false-positive alignment errors from cascading after a parse
failure.

**Source:** `model/file/utterance/parse_health.rs`

---

## Alignment Algorithm

All alignment functions follow the same pattern:

1. **Count** alignable content from the source tier (cheap count-only operation)
2. **Count** items on the target tier
3. **Pair** 1:1 for `min(source_count, target_count)` indices
4. If counts differ:
   a. **Extract** full item text (lazy -- only computed for error messages)
   b. **Format** mismatch error with positional or LCS-based diff
   c. **Add placeholder** pairs for extra items on whichever side is longer
5. Return alignment with collected pairs and errors

**Source:** `alignment/helpers/count.rs`, `alignment/helpers/rules.rs`,
`alignment/format.rs`

---

## Orchestration

`Utterance::compute_alignments()` is the entry point that runs all alignment
checks for a single utterance. It:

1. Resets all embedded alignment state
2. Checks `ParseHealth` for each tier pair
3. Calls the appropriate alignment function for each present tier
4. Stores results in `AlignmentSet` (which has `Option` fields for each tier)
5. Collects all errors into `self.alignment_diagnostics`

The results are then reported during `Utterance::validate()` through the
`ErrorSink`.

**Source:** `model/file/utterance/metadata/alignment.rs`

---

## Error Code Summary

| Code | Variant                   | Meaning                                                   |
|------|---------------------------|-----------------------------------------------------------|
| E604 | `GraWithoutMor`           | `%gra` tier present without `%mor` tier                   |
| E705 | `MorCountMismatchTooFew`  | `%mor` has fewer items than main tier alignable content    |
| E706 | `MorCountMismatchTooMany` | `%mor` has more items than main tier alignable content     |
| E707 | (inline)                  | Terminator mismatch between main and `%mor`               |
| E712 | `GraInvalidWordIndex`     | `%gra` word index out of bounds or `%mor` chunks > `%gra` relations |
| E713 | `GraInvalidHeadIndex`     | `%gra` head index out of bounds or `%gra` relations > `%mor` chunks |
| E714 | `PhoCountMismatchTooFew`  | `%pho`/`%mod`/`%wor` has fewer tokens than main tier      |
| E715 | `PhoCountMismatchTooMany` | `%pho`/`%mod`/`%wor` has more tokens than main tier       |
| E718 | `SinCountMismatchTooFew`  | `%sin` has fewer tokens than main tier                    |
| E719 | `SinCountMismatchTooMany` | `%sin` has more tokens than main tier                     |
| E720 | `MorGraCountMismatch`     | General `%mor`/`%gra` count mismatch                      |

---

## Source Files

| File                                         | Purpose                                  |
|----------------------------------------------|------------------------------------------|
| `alignment/mod.rs`                           | Module root; alignment rule summary      |
| `alignment/mor.rs`                           | Main -> `%mor` alignment                 |
| `alignment/pho.rs`                           | Main -> `%pho` alignment                 |
| `alignment/sin.rs`                           | Main -> `%sin` alignment                 |
| `alignment/wor.rs`                           | Main -> `%wor` alignment                 |
| `alignment/gra/align.rs`                     | `%mor` -> `%gra` alignment               |
| `alignment/helpers/domain.rs`                | `AlignmentDomain` enum                   |
| `alignment/helpers/rules.rs`                 | Core alignability rules per domain       |
| `alignment/helpers/count.rs`                 | Counting logic for all domains           |
| `alignment/format.rs`                        | Mismatch error message formatting        |
| `model/file/utterance/metadata/alignment.rs` | `compute_alignments()` orchestrator      |
| `model/file/utterance/parse_health.rs`       | `ParseHealth` gating                     |
| `model/alignment_set.rs`                     | `AlignmentSet` storage type              |

All paths are relative to `crates/talkbank-model/src/`.

---

Last Updated: 2026-02-12
