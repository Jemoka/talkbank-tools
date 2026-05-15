# L2 Morphotag: Current Status

**Status:** Current
**Last updated:** 2026-05-05 13:54 EDT

> **L2 dispatch is now on by default.** Aggregate
> evaluation across 19 language pairs (17 at 100% dispatch; `cym,eng`
> at 99.8% and `eng,yue,zho` at 99.9%; **99.96% aggregate**) triggered
> removal of `--experimental-l2-morphotag` and addition of
> `--no-l2-morphotag` (opt-out).

## What's Done

### Feature: L2 dispatch (default; opt out via `--no-l2-morphotag`)

Routes @s (code-switched) words to secondary language Stanza models
and merges the results with the primary model's structural analysis.
Replaces `L2|xxx` with real morphological analysis.

**Quality:** ~95% acceptable on German-English (hogan2), ~90% on
Spanish-English (herring12), ~97% on French-Dutch (Anouk). 100% splice
rate (zero L2|xxx remaining when flag is on).

### Architecture

```text
morphosyntax/l2/
├── deprel.rs   — UdDeprel newtype, deprel→POS constraint mapping
├── plan.rs     — contiguous span planning + host attachment planning
├── merge.rs    — POS resolution (6-level priority), planned Mor-based merge
├── extract.rs  — primary structural info extraction from UD responses
├── spans.rs    — contiguous span grouping for secondary dispatch
├── splice.rs   — splice merged Mor into ChatFile
└── tests.rs    — unit-test coverage for merge/splice/dispatch behavior
```

**Dispatch** (`batch.rs:dispatch_secondary_l2`):
- Plan per-utterance contiguous spans and host attachments in `plan.rs`
- Dispatch to secondary Stanza workers via `infer_batch`
- Map responses via `map_ud_sentence` (handles MWT Range tokens)
- Merge with primary structural info via `merge_planned_secondary_span`
- Splice into ChatFile via `splice_l2_into_chat`

All 3 code paths wired: batch, single-file pipeline, incremental.

### Key Design Decisions

1. **POS resolution priority:** copula check → constraint agreement →
   closed-class function word override → NOUN/PROPN override →
   primary structural fallback → constraint best guess

2. **Secondary model's NOUN/PROPN always trusted** over primary deprel
   constraint (primary assigns wrong deprels to foreign words)

3. **GRA correction:** when resolved POS contradicts primary deprel,
   infer correct deprel from POS + head POS

4. **UdDeprel newtype:** typed distinction between UD lowercase and
   CHAT uppercase deprel labels

### Tests

- focused Rust unit tests for planning, merge, splice, and phrasal-verb behavior
- ML golden tests for eng-spa, deu-eng, contractions, and flag-off regression

### Documentation

- `l2-morphotag.md` — design, architecture, Mermaid diagrams
- `l2-morphotag-literature.md` — 11-citation literature survey
- `l2-eval-runs/` — aggregate ungating evidence (per-pair and per-word CSVs from the evaluation suite)

### Input policy and repair tooling

- E255 now rejects whole-utterance same-language all-`@s` patterns at
  validation time; transcripts should use `[- lang]`.
- E254 is warn-only when explicit `@s:LANG` names a language absent from
  `@Languages`; dispatch still uses the explicit target language.
- `chatter debug fix-s` now repairs both transcript-side issues: it rewrites the
  qualifying whole-utterance `@s` pattern, appends missing explicit languages
  to `@Languages`, and leaves already-correct files untouched.

## What's Not Done

(No known open items. The phrasal-verb gap listed here previously was
resolved — see below.)

## Recently Fixed

### Phrasal-verb recognition — FIXED

Stanza returns `compound:prt` for true verb-particle constructions
(`wake up`, `give up`, `figure out`), but the L2 merge algorithm used
to process each `@s` word in isolation and could not see that relation.
Two consequences:

1. When the primary parser tagged a foreign verb with `advmod` (common
   for German parsing English), the deprel constraint rejected the
   secondary's VERB at Priority 2 and downgraded the head to ADV
   (e.g. `give@s up@s` → `adv|give adp|up`).
2. The particle's UPOS ADP was trusted by Priority 3 (closed-class) as
   `adp|up`, not the CHAT-conventional `part|up`.

**Fix.** `merge_primary_secondary_with_context` now accepts a
`SecondaryUdContext { sentence, word_position }` and checks:

- the current word is a phrasal-verb particle (its own deprel is
  `compound:prt`) → promote UPOS to `Part`, set `corrected_deprel` to
  `compound:prt` so the CHAT %gra tier becomes `COMPOUND-PRT`;
- the current word is a phrasal-verb head (some sibling has deprel
  `compound:prt` with head pointing to this word) and the secondary
  UPOS is Verb → keep Verb, overriding the primary constraint.

Priority 0 runs before the existing priority chain, mirroring the
Priority 4 NOUN/PROPN override that is already in place for content
nouns. No Python changes, no cache-key changes.

**Evidence.** Running the pre-fix vs post-fix binary on a German-English
fixture:

```text
Before: die kinder give@s up@s immer  →  adv|give-Fin-Imp-S adp|up
After:  die kinder give@s up@s immer  →  verb|give-Fin-Imp-S part|up
```

See `scripts/l2-eval/probe_phrasal_verbs.py` for the isolated Stanza
probe that anchored the test expectations.

**Test coverage.**

- `crates/batchalign/src/chat_ops/morphosyntax_ops/l2/tests.rs` — four unit
  tests exercising each merge branch (particle promotion, head
  promotion, non-phrasal ADP regression, non-VERB secondary safety).
- `crates/batchalign/tests/ml_golden/morphotag/golden_l2.rs::golden_l2_morphotag_phrasal_verbs`
  — end-to-end ML golden test on `wake up` / `give up` / `pick up` /
  `time out`, asserting `verb|X part|up` for the first three and
  `noun|time adp|out` for the (non-phrasal) compound noun.

## Earlier Fixes

### MWT Hint Preservation Regression — FIXED

A follow-up Python regression in `python/batchalign/inference/_tokenizer_realign.py`
silently stripped Stanza's `(text, True)` MWT hint tuples before the Rust
char-DP aligner saw them. Stanza's tokenizer natively emits those tuples for
English contractions, and its MWT processor relies on them to expand Range
tokens. With the hint gone, MWT never fired and L2 contractions regressed
despite an earlier Rust-side fix being present.

**Fix:** `_realign_sentence` / `_conform` now overlay Stanza's own tuples onto
aligner output where lengths match and no merging happened. Applies to every
language in `MWT_LANGS`.

**Evidence — 4 L2 ML-golden tests all pass:**

- `golden_l2_morphotag_eng_contractions` — `it's@s:eng` → `pron|it~aux|be`,
  `don't@s:eng` → `aux|do~part|not`
- `golden_l2_morphotag_eng_spa` — Spanish-English code-switching, ~90%
  acceptable
- `golden_l2_morphotag_deu_eng` — German-English, ~95% acceptable
- `golden_l2_morphotag_off_produces_l2_xxx` — flag-off regression guard

The prior "MWT blocker" note in this document is LIFTED.

### Interaction with the English grammatical-invariant rewrite

A new Rust rewrite rule (see
[Stanza Limitations — Defect 1](stanza-limitations.md)) runs on the **primary**
English UD analysis to fix Stanza's copula-vs-possessive failure
(`the sink's overflowing`). L2 extraction operates on the ORIGINAL
`ud_responses` — captured in `pipeline/morphosyntax.rs:352-356` and
`batch.rs` before the rewrite is applied — so the English rewrite cannot
corrupt L2 position mapping. The two features are decoupled by design.

## Earlier Fixes (MWT contraction)

### MWT Contraction Expansion — FIXED

English contractions (`it's@s`, `don't@s`) now get proper clitic
morphology: `pron|it~aux|be`, `aux|do~part|not`.

**Root cause:** Three bugs prevented MWT expansion in the L2 path:
1. `"en"` missing from `MWT_LANGS` → English pipeline loaded without
   MWT processor (dead English-specific branch in `_stanza_loading.py`)
2. Rust `inject.rs` included Range parent tokens in token vector →
   MOR count mismatch → `retokenize_utterance()` failed
3. `map_ud_sentence()` merged Range components into clitics, wrong for
   the Retokenize path where each component needs its own MOR item

**Fix:** Added `"en"` to `MWT_LANGS`, filtered Range parents from token
vector, new `map_ud_sentence_expanded()` for the Retokenize path, and
flipped `retokenize=false` to `true` in the L2 secondary dispatch
(`batch.rs:dispatch_secondary_l2`).

**Golden test:** `golden_l2_morphotag_eng_contractions` verifies
`it's@s:eng` → `pron|it~aux|be` and `don't@s:eng` → `aux|do~part|not`.

### Primary `--retokenize` for non-CJK — FIXED

The `--retokenize` flag now works for English (and all MWT languages).
`golden_morphotag_retokenize_eng` shows expanded output matching BA2:
`gonna eat cookies .` → `gon na eat cookies .` with per-component MOR.
