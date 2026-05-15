# Stanza Limitations — Observed Defects with Version Pinning

**Status:** Reference (living document — update when Stanza behavior changes)
**Last updated:** 2026-05-14 23:31 EDT
**Current Stanza pin:** `stanza[transformers]>=1.12.0` (see `pyproject.toml`)
**Current English MWT package:** `gum`

> **2026-05-14 — Stanza 1.12.0 upgrade:** every defect below was
> re-evaluated against Stanza 1.12.0. Verdicts: defect 4 (Finnish
> `<SOS>` leak) is **Fixed upstream** and its mitigation was
> removed; defects 1, 2, 5, 6, 7, 8 are still confirmed and have
> `1.12.0` added to their version lists; defect 3 (CJK reference-only)
> was not re-evaluated this pass.

> **See also:**
> [Stanza Defect Mitigation Map](../architecture/stanza-defect-mitigation-map.md)
> — pipeline-stage view of where each defect below is patched.

## Purpose

Stanza is a third-party dependency whose analyses drive much of BA3's
NLP output (morphosyntax, utterance segmentation, dependency parsing).
Stanza is imperfect. When Stanza's output is wrong, BA3 has to either
override it with principled rules or accept degraded output. Either
choice creates technical debt: overrides can go stale if Stanza
improves, and accepted-wrong output misleads users.

This document records Stanza defects we have observed, with the
Stanza version in which each defect was confirmed. When Stanza is
upgraded, this document is the re-evaluation checklist: re-run the
permanent tests associated with each defect and, if Stanza has fixed
it, remove the BA3 override.

**Core principle:** BA3 targets linguistic correctness, not Stanza
parity. When Stanza is wrong, BA3 overrides. When Stanza improves,
BA3 un-overrides. Both directions are driven by tests that pin the
observed behavior.

## Format for each entry

* **Defect:** brief name + construction class affected
* **Stanza version:** where confirmed
* **Input example:** minimal reproducible sentence
* **Stanza output:** what it currently produces (POS, lemma, deprel)
* **Correct output:** what it should produce, grounded in
  linguistic/CHAT conventions
* **BA3 mitigation:** the principled override we apply, with a
  pointer to the implementation
* **Tests:** the permanent tests that lock this observation
* **Re-evaluation criteria:** what to check when Stanza upgrades

---

## Defect 1: Copula `'s` vs possessive `'s` disambiguation fails before nominal gerunds

* **Stanza version:** 1.10.1, 1.11.1, and 1.12.0 (all confirmed; re-verified by `test_stanza_mwt_copula_observations.py` against 1.12.0 on 2026-05-14)
* **MWT package:** `gum`
* **Construction:** `<noun>'s <word-ending-in-ing>` in a main clause.

### Input examples

```text
and the sink's overflowing .
the lady's washing dishes .
```

### Stanza's output

Stanza commits to a possessive reading end-to-end:

| Word | upos | lemma | deprel | Head |
|------|------|-------|--------|------|
| sink | NOUN | sink | nmod:poss | overflowing |
| 's | PART | 's | case | sink |
| overflowing | NOUN | overflow | root | 0 |

The whole sentence is parsed as the noun phrase "the sink's
overflowing" with "overflowing" as the nominal head. This leaves the
main clause with **no finite verb** — ungrammatical English.

### Correct output

The sentences are contracted copula `is` + progressive. The `'s`
should be AUX with lemma `be`, and the `-ing` word should be a verbal
present participle:

| Word | upos | lemma | deprel | Head |
|------|------|-------|--------|------|
| sink | NOUN | sink | nsubj | overflowing |
| 's | AUX | be | aux | overflowing |
| overflowing | VERB | overflow | root | 0 |

CHAT `%mor` target: `noun|sink~aux|be-Fin-Ind-Pres-S3 verb|overflow-Part-Pres-S`.

### Why Stanza gets this wrong

The construction `<noun>'s <-ing>` is ambiguous in isolation:

* **Copula reading** (correct in conversational CHAT): "the sink is
  overflowing" — `<noun>` is the subject, `is` is the finite verb,
  `-ing` is present participle in progressive aspect.
* **Possessive reading** (rare, requires broader clausal context):
  "the sink's overflowing `[is problematic]`" — `<noun>'s` is possessor,
  `-ing` is a deverbal gerund noun.

In natural English conversation, the copula reading is overwhelmingly
more common. Human speakers rely on prosody, context, and the grammatical
requirement that main clauses have a finite verb to resolve the
ambiguity. Stanza's POS tagger does not reliably use sentence-level
well-formedness as a tiebreaker, and in the failure cases its tagger
commits to the possessive reading even when the result is an
ungrammatical fragment.

Counter-examples (Stanza handles correctly, for reference):

* `he's falling over` — Stanza correctly tags `'s` as AUX. The
  pronoun `he` probably helps — `his` would be the possessive form.
* `the stool's going over` — Stanza correctly tags `'s` as AUX.
  `going` gets tagged as VERB (VerbForm=Part) here, perhaps because
  "going" is more verb-like in its training than "overflowing" /
  "washing".

The failing cases (`sink`, `lady`) share the pattern: a NOUN head and
an `-ing` form that Stanza's lexical semantics considers
noun-compatible (overflowing as a noun-like event; washing as in
"washing machine"). Whatever Stanza's internal signal is, it fails
for these.

### BA3 mitigation (ACTIVE)

**Grammatical-invariant rewrite on typed UD data.** The "main clauses
require a finite verb" invariant is checked on each UdSentence before
`map_ud_sentence` runs; when violated AND an MWT-bound `'s` tagged as
PART/case is present AND exactly one NOUN-tagged `-ing` word exists,
the rule rewrites the sentence into its coherent copula-progressive
analysis (flipping `'s` to AUX/be/Fin, promoting the `-ing` word to
root VERB/VerbForm=Part, reattaching subject and object dependencies).
Handles two sub-patterns:

* **Pattern A (root == target):** the `-ing` word is already Stanza's
  root (e.g., `and the sink's overflowing`). In-place POS/feat flip
  plus subject reattachment.
* **Pattern B (root != target):** the `-ing` word is a compound
  modifier; a different noun holds the root position (e.g., `the
  lady's washing dishes` — Stanza makes `dishes` root). The `-ing`
  word is promoted to root, the former root is demoted to `obj`,
  subject and punctuation are reattached.

Implementation: `crates/talkbank-transform/src/morphosyntax/invariants/finite_verb_main_clause.rs`.
Dispatcher: `crates/talkbank-transform/src/morphosyntax/invariants.rs`.
Hook point: `crates/talkbank-transform/src/morphosyntax/injection.rs:142`
(call site immediately before `map_ud_sentence`).

### Tests

**Rust unit tests** (14 tests, all GREEN):
`crates/talkbank-transform/src/morphosyntax/invariants/finite_verb_main_clause.rs`
`#[cfg(test)]` block — 2 positive rewrite tests (sink pattern A, lady
pattern B), 10 negative no-op tests covering distinct precondition
failure modes.

**Python end-to-end tests** (new file, 2 tests, all GREEN):
`batchalign/tests/pipelines/morphosyntax/test_preserve_mwt_end_to_end.py`
— runs `batchalign3 morphotag` on a CHAT fixture with all four
copula-contraction sentences, asserts final `%mor` contains
`~aux|be-Fin-Ind-Pres-S3` for every one and `%gra` contains the
corresponding AUX/NSUBJ/ROOT structure.

**Python observation tests** (retained as versioned documentation of
Stanza's current output): `test_stanza_mwt_copula_observations.py`,
`test_preserve_mwt.py` — assert what Stanza emits at the
intermediate layer (still PART/'s for sink/lady; BA3 corrects
downstream).

### Re-evaluation criteria (when Stanza upgrades)

On Stanza upgrade:

1. Temporarily disable the rewrite (return `sentence.clone()`
   unconditionally in `rescue_english_copula_progressive`).
2. Re-run the Python observation tests. If Stanza now emits AUX/be
   for sink/lady, the underlying Stanza defect is fixed; the
   mitigation can be removed. Update this document with the Stanza
   version and remove the rule and its tests.
3. If the Python observation tests still show PART/'s, re-enable the
   mitigation and update the Stanza version header at the top of
   this document.

### Long-term successor (Option D — fine-tune Stanza)

The rewrite is a principled compromise, not the ideal solution. The
correct long-term fix is to retrain Stanza's English POS and
depparse models on data where contracted copula `'s` before a
present participle is labeled correctly. Work items:

* Build a CHAT-to-UD conversion pipeline that labels MWT Range `'s`
  in copula-progressive contexts as AUX/be/Fin (and the following
  `-ing` word as VERB/VerbForm=Part).
* Curate a training set from BA3's ~199 MB of CHAT transcripts —
  English Clinical and English CHILDES-NA corpora have the target
  construction in abundance.
* Set up a Stanza continued-training job from the published
  checkpoint.
* Evaluate against both a held-out CHAT test set (must improve) and
  Stanza's default English test set (must not regress).
* Distribute: either bundle a custom Stanza model with BA3 or
  publish to the Stanza model hub.
* Once deployed, follow the re-evaluation procedure above to retire
  the invariant rewrite.

Estimated effort: weeks to months. Tracked here as a future work
item; not scheduled.

---

## Defect 2: MWT hint tuples must be preserved through postprocessors (Stanza/Python interop gotcha)

* **Stanza version:** 1.10.1, 1.11.1, and 1.12.0 (all confirmed; re-verified by `test_stanza_mwt_copula_observations.py` against 1.12.0 on 2026-05-14)
* **Nature:** Not strictly a Stanza bug — a contract that the
  `tokenize_postprocessor` API places on callers but does not document
  prominently. Easy to violate in a wrapper that flattens tuples to strings.

### Summary

Stanza's tokenizer natively emits `(text, True)` two-element tuples for
English contractions (and other MWT-capable languages). Its MWT processor
honors those hints to expand the token into Range components (`don't` →
`do` + `n't`). Any `tokenize_postprocessor` callback that discards the
boolean — for example by extracting only `tok.text` before a downstream
aligner runs — silently disables MWT expansion for the whole document.
The symptom is subtle: no error, no warning, just missing Range tokens
and therefore no `~`-joined `%mor` output for English contractions.

### Why this matters in BA3

If `python/batchalign/inference/_tokenizer_realign.py::_realign_sentence`
flattens tokens to plain strings before passing them to the Rust
char-DP aligner, the aligner's 1:1 mapping (the common case where no
compound-merging is needed) loses the hint tuple, and Stanza's MWT
processor sees only bare strings. MWT never fires, all English
contractions regress to single-token morphology, and the symptom is
silent (no error, no warning).

### BA3 mitigation (ACTIVE)

`_realign_sentence` now overlays Stanza's original `(text, True)` tuples
onto the aligner output for positions where lengths match and no merging
happened. The hint survives the realignment and reaches Stanza's MWT
processor intact. Applies to every language for which the runtime
capability table reports `has_mwt=True` (see Defect 5 for how MWT
availability is decided per language).

### Tests

* **L2 ML-golden tests:**
  `golden_l2_morphotag_eng_contractions`, `golden_l2_morphotag_eng_spa`,
  `golden_l2_morphotag_deu_eng`, `golden_l2_morphotag_off_produces_l2_xxx`.
* **Python observation tests:** `test_stanza_mwt_copula_observations.py`
  pins Stanza's native tuple emission for the four copula-contraction
  fixtures.

### Re-evaluation criteria (when Stanza upgrades)

Stanza is unlikely to move away from the tuple convention — it is a
documented API contract. The re-check on upgrade is:

1. Re-run `test_stanza_mwt_copula_observations.py`. It still
   requires Stanza to emit `(text, True)` for English contractions.
2. If a future Stanza version emits a different hint shape (e.g., a
   dedicated class instead of a tuple), update `_realign_sentence`'s
   overlay logic to recognize the new shape. The principle — preserve
   Stanza's hints through our wrapper — does not change.

---

## Defect 3: CJK tokenization and POS quality (reference only — existing workarounds)

* **Stanza version:** 1.10.x and 1.11.x (both)
* **Construction:** Word segmentation and POS tagging for Chinese
  (Mandarin, Cantonese) and Japanese.
* **Symptom:** Stanza's accuracy on conversational CJK text is below
  what BA3 needs for CHAT-quality morphotag output, especially for
  Cantonese.
* **BA3 mitigation:** BA3 uses dedicated engines for CJK — PyCantonese
  for Cantonese POS, unified Stanza training on HKCanCor+UD for
  Cantonese tokenize+POS+depparse, and pretokenize+CHAT-gold
  segmentation for Japanese.
* **Tests:** `batchalign/tests/pipelines/morphosyntax/test_cantonese_*`,
  `test_stanza_cantonese_*`, `test_mandarin_*`.
* **Re-evaluation criteria:** When Stanza ships a CJK model upgrade,
  re-run the baseline accuracy tests and compare against the
  per-engine quality benchmarks.

This entry is listed for completeness; it belongs in the same registry.

---

## Defect 4: Neural-LM control tokens leak into Document output (Finnish MWT)

<a id="stanza-fi-mwt-sos-leak"></a>

* **Stable slug** (use this in code references — defect numbers can
  renumber as entries are retired): ``stanza-fi-mwt-sos-leak``
* **Stanza version:** 1.11.1 (confirmed); older versions not tested. **Fixed in Stanza 1.12.0** — verified by `test_stanza_fi_mwt_sos_leak.py` GREEN on 2026-05-14, and by the engineered fail-loudly signal in `test_control_token_leak_propagation.py`. The strip+warn mitigation was removed in the 1.12.0 upgrade commit; the standalone reproducer is retained as a regression sentinel.
* **Nature:** Character-level language-model internal tokens (``<SOS>``,
  start-of-sequence) leak into Stanza's public ``Document`` API,
  appearing as literal substrings on ``word.text`` and ``word.lemma``.
  Observed on Finnish when the MWT processor splits the word
  ``tollei`` in a 3+-word context.

### Input examples

Minimum trigger (no domain knowledge of Finnish needed):

```text
a tollei b
```

Larger real-corpus example (from
`childes-other-data/Finno-Ugric/Finnish/Kirjavainen-MPI/1-08-01.cha`
line 2222):

```text
*MOT:  kato se on tommonen (.) tollei se menee xxx .
```

### Stanza's output

```text
token.text='a'      word.text='a'        lemma='a'        upos=NOUN
token.text='tollei' word.text='<SOS>tos' lemma='<SOS>tos' upos=SYM   ← LEAK
                    word.text='ei'       lemma='ei'       upos=VERB
token.text='b'      word.text='b'        lemma='b'        upos=NOUN
```

The MWT expansion is correct in shape (``tollei`` → ``tos`` + ``ei``),
but the first expansion word has ``<SOS>`` prepended on both its
``text`` and ``lemma`` fields. The second expansion word is clean.

### Correct output

```text
word.text='tos'  lemma='tos'  upos=SCONJ
word.text='ei'   lemma='ei'   upos=VERB
```

### Why the leak matters

Batchalign writes Stanza's output to CHAT ``%mor`` tiers. The CHAT
manual's ``%mor`` grammar does not permit angle-bracket content
inside stems, so an unguarded leak produces invalid CHAT like
``sconj|<sos>tos~aux|ei-Fin-Neg-S3`` that ``chatter validate``
correctly rejects with E316. The rejection is a reliable final
safety gate but inconvenient — the corruption ships to disk before
validation sees it.

Observed impact: files in CHILDES Finnish Kirjavainen-MPI have
carried this pattern in committed state, traceable to morphotag runs.

### Trigger conditions

Empirically narrowed down from the original 8-word sentence:

* Language: Finnish (``lang="fi"``)
* Processors include MWT: ``tokenize,pos,lemma,depparse,mwt``
* ``tollei`` appears as a non-boundary token (3+ whitespace-separated
  tokens total; ``se tollei`` and ``tollei se`` do NOT leak)

The leak is not observed for arbitrary Finnish MWT splits — it
reproduces reliably on ``tollei`` but was not seen on the other
Finnish MWTs we sampled. Likely a corner case in the character LM's
interaction with the MWT processor on this specific surface form.

### BA3 mitigation (ACTIVE)

Detect control tokens at the Python/Rust ingress boundary and strip
them in place, logging each rewrite with a ``tracing.warning``. The
stripper is in
``python/batchalign/inference/_control_token_filter.py::strip_control_tokens_in_sentence``;
the call site is
``python/batchalign/inference/morphosyntax.py::batch_infer_morphosyntax``
right after ``doc.to_dict()`` yields the Stanza sentence.

Control-token vocabulary covered: ``<SOS>``, ``<EOS>``, ``<UNK>``,
``<PAD>``, ``<BOS>``, ``<CLS>``, ``<SEP>``, ``<MASK>``, ``<s>``,
``</s>``. Case-insensitive to catch both tokenizer-stage ``<SOS>``
and lemmatizer-lowercased ``<sos>`` (Stanza emits both variants on
the same leak).

The workaround mirrors Defect 1 and Defect 2: detect → rewrite →
log → register → test. It does NOT silently swallow the defect:
every rewrite is visible in server logs as a warning, so an ops
reader monitoring the fleet can confirm the workaround still fires
on each Stanza version.

### Tests

* **Standalone upstream reproducer:**
  ``batchalign/tests/pipelines/morphosyntax/test_stanza_fi_mwt_sos_leak.py``
  — no batchalign imports, safe to copy into an upstream Stanza
  issue tracker submission.
* **Pure-function unit tests (34 tests):**
  ``batchalign/tests/inference/test_control_token_filter.py``
  — pin the regex vocabulary and the strip-in-place contract.
* **Integration test:**
  ``batchalign/tests/pipelines/morphosyntax/test_control_token_leak_propagation.py``
  — exercises ``batch_infer_morphosyntax`` end-to-end on a live
  Finnish Stanza pipeline, asserts the UD response is clean and a
  warning was logged.

### Re-evaluation criteria (when Stanza upgrades)

1. Re-run the standalone reproducer
   (``test_stanza_fi_mwt_sos_leak.py``). If Stanza's Finnish MWT
   no longer leaks ``<SOS>`` on ``"a tollei b"``, the upstream bug
   is fixed.
2. If the standalone reproducer is GREEN on the new Stanza, the
   integration test will still pass (no leak to strip = no warning).
   At that point the strip + warning code becomes dead for this
   defect; retire it only after confirming no other language
   exhibits a similar leak on Stanza's new version.
3. If the standalone reproducer is still RED but on a different
   surface form, extend the regex vocabulary if the new token type
   falls outside the current list; otherwise no code change needed.

### Upstream reporting

An issue has not yet been filed. The standalone reproducer is
prepared and ready to submit. Until it is filed and resolved, the
BA3 workaround stays in place.

---

## Defect 5: MWT processor selection must come from the live capability table, not a hardcoded mirror

<a id="stanza-mwt-capability-driven-selection"></a>

* **Stable slug:** ``stanza-mwt-capability-driven-selection``
* **Stanza version:** every 1.x release through 1.11.1; the issue is
  the BA3 loader, not Stanza.
* **Nature:** A loader-side bug, not a Stanza bug. BA3 maintained a
  hardcoded ``MWT_LANGS`` include set in
  ``python/batchalign/worker/_stanza_loading.py`` that drifted from Stanza's
  installed catalog. For any language on the include set that Stanza
  did not actually ship MWT for, the worker requested an unavailable
  processor and crashed at bootstrap with
  ``UnsupportedProcessorError``. Swedish was the case that surfaced
  the bug; the underlying class affects every language.

### Symptom (Swedish)

When the loader requests ``processors="tokenize,pos,lemma,depparse,mwt"``
for Swedish, the Stanza pipeline never finishes loading. The Python
worker subprocess prints the bootstrap traceback and exits before
emitting a ``ready`` signal, the Rust dispatcher reports
``Batch infer failed for language group lang=swe``, and every Swedish
file in the batch is failed cleanly by the language-group failure
aggregator. No silent corruption, but no ``%mor`` either.

### History — and why this was a regression, not a new bug

BA2-jan9 (the ``84ad500`` baseline this team uses as the migration
oracle) already handled this correctly. The relevant excerpt from
``batchalign/pipelines/morphosyntax/ud.py:760-772`` in BA2-jan9:

```python
mwt_exclusion = ["hr", "zh", "zh-hans", "zh-hant", "ja", "ko",
                 "sl", "sr", "bg", "ru", "et", "hu",
                 "eu", "el", "he", "af", "ga", "da", "ro"]

elif not any(i in mwt_exclusion
             or "mwt" not in get_language_resources(resources, i)
             for i in lang):
    if "en" in lang:
        config["processors"]["mwt"] = "gum"
    else:
        config["processors"]["mwt"] = "default"
```

The right-hand side of the OR is the principled check: it asks
Stanza's installed catalog whether the language has an ``mwt``
processor before requesting it.

The left-hand ``mwt_exclusion`` list predates Ignas's commit and
carries no documented rationale per language. It is a snapshot of
"what Stanza didn't ship at some earlier point," frozen in the
source. As Stanza added MWT models for more languages (including
Hebrew, Greek, and Estonian), the list became progressively stale
but kept vetoing. By 2026-01, three of its entries (``el``, ``et``,
``he``) had been overruled in upstream Stanza but were still being
excluded by BA2's hardcoded list; the remaining entries either
agreed with the catalog or were deliberate-CJK exclusions (handled
elsewhere in BA3).

When BA3 was built, BA2's two-armed check was flattened into a
single hardcoded include set ``MWT_LANGS``. The runtime catalog arm
was lost. The hardcoded include set then drifted in the opposite
direction — listing Swedish (``sv``) for MWT even though Stanza had
never shipped a Swedish MWT model.

### BA3 fix (ACTIVE)

``MWT_LANGS`` is deleted. The loader consults the capability table
at every Stanza pipeline construction:

```python
has_mwt = should_request_mwt(alpha2, get_cached_capability_table())
```

The capability table (``python/batchalign/worker/_stanza_capabilities.py``)
is built once at worker startup from
``stanza.resources.common.load_resources_json()`` and reports per
language whether each processor is available. CLAUDE.md mandates
this pattern: *"Per-language processor availability is determined
by reading Stanza's resources.json at worker startup, NOT by
hardcoded tables. Never hardcode processor assumptions."*

Per-language behavior with this fix:

| Language | Old hardcoded list | Capability table | Net change |
|----------|-------------------|------------------|------------|
| Swedish (``sv``) | True (wrong) | False | **MWT no longer requested → bootstrap succeeds (was crashing)** |
| Hebrew (``he``) | False | True | **MWT requested where it produces real splits** |
| Greek (``el``) | False | True | **MWT requested where it produces real splits** |
| Estonian (``et``) | False | True | MWT requested but does not fire on conversational input (effective no-op; see "Estonian no-op" below) |
| Russian (``ru``) | False | False | unchanged |
| Japanese (``ja``) | False | False | unchanged |
| English / French / German / Italian / Spanish / Dutch / Finnish / etc. | True | True | unchanged |

### Practical impact on linguistic output

**Swedish (was crashing, now runs without MWT).** Stanza never
shipped a Swedish MWT model. Swedish orthography keeps most function
words separate, so the loss of MWT mostly does not affect ``%mor``
granularity. A few historical contractions (e.g. ``i+det → it``) pass
through as single tokens; this is acceptable for CHAT corpora and
matches what BA2-jan9's runtime check would also have produced
(BA2-jan9 also did not request Swedish MWT, via the right-hand arm
of its OR).

**Hebrew (now gets MWT, was being suppressed).** Stanza splits
Hebrew prepositional+definite contractions and definite-article
fusion correctly:

```text
בבית   → ב + בית         "in the house" — prep ב + noun (definite ה absorbed)
מהילד  → מ + ה + ילד      "from the boy" — prep מ + def ה + noun
לאישה  → ל + אישה         "to the woman" — prep ל + noun
הזה    → ה + זה          "this" — def ה + demonstrative
```

These are linguistically real morpheme boundaries; producing them
in ``%mor`` is the correct CHAT-format output. BA2-jan9's hardcoded
exclusion of Hebrew predates Ignas's 2024 commit and has no recorded
justification.

**Greek (now gets MWT, was being suppressed).** Stanza splits the
preposition+article contractions correctly:

```text
στο    → σ + το           "in the (n.acc)" — prep σε + def το
στον   → σ + τον          "in the (m.acc)" — prep σε + def τον
στις   → σ + τις          "at the (f.pl.acc)" — prep σε + def τις
```

Same justification as Hebrew. The split components are the
underlying lexical items; merging them into a single token loses
real morphosyntactic structure.

**Estonian no-op.** Stanza ships ``et`` MWT in
``resources.json``, so the capability table reports
``has_mwt=True`` and BA3 requests it. Empirically, Stanza's Estonian
MWT model does not split anything on conversational input — including
the contracted-negation forms (``pole`` = ei+ole, ``polnud`` =
ei+olnud, ``polegi`` = ei+ole+gi) that UD Estonian-EDT does mark as
MWT in the treebank. Probed inputs and observed Stanza output:

| Input | Expected (UD-EDT) | Stanza-1.11.1 output |
|-------|-------------------|----------------------|
| ``pole tähtis`` | ``ei+ole`` ``tähtis`` | ``pole`` ``tähtis`` (no split) |
| ``polnud aega`` | ``ei+olnud`` ``aega`` | ``polnud`` ``aega`` (no split) |
| ``ma pole näinud`` | ``ma`` ``ei+ole`` ``näinud`` | ``ma`` ``pole`` ``näinud`` (no split) |
| ``ta polegi tulnud`` | ``ta`` ``ei+ole+gi`` ``tulnud`` | ``ta`` ``polegi`` ``tulnud`` (no split) |

The Estonian MWT model likely trained on a treebank where MWT was
rarely or never marked on conversational ``pole``-class forms;
either way, requesting it has no observable effect on real CHAT
input. We request it for consistency with the capability table; if
the upstream model later starts splitting these forms, the change
will surface through a test rather than as silent output drift.

### Tests

* **Pure-function unit tests** (4 tests, GREEN — no Stanza required):
  ``batchalign/tests/pipelines/morphosyntax/test_stanza_loading.py::TestShouldRequestMwt``
  pins ``should_request_mwt`` against synthetic capability tables
  (Swedish-not-supported, English-supported, unknown-language,
  table-is-None).
* **Live-catalog regression tests** (3 tests, GREEN):
  ``batchalign/tests/pipelines/morphosyntax/test_stanza_config_parity.py::TestMwtCapabilityDriven``
  asserts the runtime decision against the actual installed Stanza
  catalog — Swedish False, English True — and uses an AST scan to
  forbid re-introduction of a hardcoded ``MWT_LANGS`` set.
* **Hebrew/Greek MWT split observation tests** (golden, real Stanza):
  ``batchalign/tests/pipelines/morphosyntax/test_stanza_he_el_mwt_splits.py``
  pins the linguistically-correct splits for the canonical Hebrew
  and Greek constructions listed above. Standalone — no batchalign
  imports — safe to share with upstream if Stanza output drifts.
* **CHAT end-to-end tests** (golden, integration):
  ``batchalign/tests/pipelines/morphosyntax/test_he_el_mwt_end_to_end.py``
  runs ``batchalign3 morphotag --sequential`` on minimal Hebrew and
  Greek CHAT fixtures, asserts ``%mor`` contains tilde-joined splits
  for the contraction forms, and verifies the output passes
  ``chatter validate``.

### Re-evaluation criteria (when Stanza upgrades)

1. Re-run the live-catalog tests. If ``should_request_mwt`` flips
   for any language, the capability table picked up an upstream
   change automatically; no BA3 code change needed.
2. Re-run the Hebrew/Greek split tests. If Stanza's splits change
   shape, decide whether the new shape is linguistically defensible
   and update the assertions, or file an upstream issue.
3. Re-probe Estonian. If Stanza's Estonian MWT begins splitting
   ``pole``-class forms, update the table above and add positive
   assertions.

### Why we did not re-introduce a deliberate-exclude list

BA2-jan9's hardcoded ``mwt_exclusion`` list was inherited without
documented rationale (only Ignas's runtime-check arm had a clear
purpose). Recreating it in BA3 would re-create the same drift bug
in a different shape — over time, the upstream catalog moves and
the hardcoded list goes stale. Per the project's standing policy
("BA2 is known-buggy; never duplicate BA2's wrong output"), the
correct stance is to trust the capability table and let any
linguistically-bad MWT splits surface through tests.

### Long-term: Swedish MWT

If Swedish corpus throughput justifies it, train a Stanza MWT model
on UD Swedish-Talbanken or UD Swedish-LinES and contribute it
upstream. The capability table would then flip automatically on the
next Stanza release and Swedish would gain MWT support with no BA3
code change. Estimated effort: weeks; not currently scheduled.

Routing Swedish to a sibling model (Norwegian-Bokmål) is **not** a
viable workaround — Norwegian and Swedish are distinct languages,
per-word morphological features would be wrong, and the resulting
``%mor`` would be misleading rather than merely incomplete.

---

## Defect 6: Italian POS layer splits words with clitic-shaped endings into fake verb+clitic compounds

<a id="stanza-it-verb-clitic-pos-split"></a>

* **Stable slug:** ``stanza-it-verb-clitic-pos-split``
* **Stanza version:** 1.11.1 and 1.12.0 (both confirmed; re-verified via MWT probe matrix `xfail` markers held on 2026-05-14)
* **MWT package:** Italian default
* **Failure class:** linguistic-content quality. Stage 3's MWT Range
  reassembly
  (`crates/talkbank-transform/src/morphosyntax/mapping_helpers.rs::assemble_mors`)
  collapses Stanza's 2-word expansion into a single compound `%mor`
  entry per CHAT word using `~` / `+`, so the count invariant holds.
  The content of that `%mor` entry is what's wrong: a fake verb
  lemma plus a spurious enclitic pronoun.
* **Construction:** Italian words whose last one-to-two characters
  match a clitic shape (`-la`, `-lo`, `-le`, `-li`, `-ne`, `-no`,
  `-ni`, `-mi`, `-ti`, `-ci`, `-vi`, `-si`) are wrapped by Stanza in
  an MWT Token and analyzed as `verb stem + enclitic pronoun` —
  with a bogus stem lemma — regardless of actual part of speech.
  The defect fires not only on verb forms like `parla` (imperative
  of `parlare`) but also on common nouns like `arancione` (orange),
  `seggiola` (chair), `gomitolo` (ball of yarn), `cavallone` (big
  horse), `cielo` (sky), `bottone` (button), on adjectives like
  `piccolo` / `piccola` (small), and on diminutive/augmentative
  baby-talk forms (`coccole`, `babbolo`, `pettole`). Most of the
  non-verb hits are tagged with `Part Past` features — Stanza
  confidently treats the whole surface as a past participle plus
  clitic.

### Representative examples and observed end-to-end `%mor` output

Pulled from the ita-only corpus audit
(`scripts/analysis/audit_italian_mor_content.py`, pointed via
`--root` or `$TB_DATA_JSON` at a pre-parsed JSON snapshot of the
TalkBank CHAT corpora):

| Surface (actual meaning)       | Shipped `%mor`                                            | Features on stem |
|--------------------------------|-----------------------------------------------------------|------------------|
| `parla` (speak!, imp)          | `verb\|par~pron\|la`                                      | `Fin Imp Pres S2` |
| `arancione` (orange, noun)     | `verb\|arancio~pron\|ne`                                  | `Part Past`      |
| `seggiola` (little chair)      | `verb\|seggio~pron\|la`                                   | `Part Past`      |
| `piccolo` (small, adj)         | `verb\|picco~pron\|lo`                                    | `Part Past`      |
| `piccola` (small, fem adj)     | `verb\|picco~pron\|la`                                    | `Part Past`      |
| `gomitolo` (ball of yarn)      | `verb\|gomito~pron\|lo`                                   | `Part Past`      |
| `divano` (sofa)                | `verb\|diva~pron\|no`                                     | `Part Past`      |
| `cielo` (sky)                  | `verb\|cie~pron\|lo` (`cie` is not a word)                | `Inf Ind Imp S2` |
| `bottone` (button)             | `verb\|botto~pron\|ne`                                    | `Part Past`      |
| `cavallone` (big horse)        | `verb\|cavallo~pron\|ne`                                  | `Part Past`      |

Every row has ONE `%mor` item for ONE CHAT word — the Range
reassembly worked, the count invariant holds. Every row's content
is wrong.

### Correct output (illustrative)

```text
parla     → verb|parlare-Imp-S2        (2sg imperative of parlare)
arancione → adj|arancione OR noun|arancione
seggiola  → noun|seggiola-Fem-Sing     (little chair)
piccolo   → adj|piccolo-Masc-Sing      (small, m.sg)
gomitolo  → noun|gomitolo-Masc-Sing    (ball of yarn)
```

Italian UD analyses of these forms are well-defined; Stanza's
Italian POS/MWT model fails to produce them for a class of
clitic-shaped endings.

### Why no tokenizer hack rescues it

The split happens at the **POS/depparse layer**, not at tokenize.
`batchalign`'s `tokenize_postprocessor` hook runs during
Stanza's tokenize stage and sees a single token `parla`. The split
is introduced later by Stanza's POS tagger interpreting `parl-` as
the imperative stem and producing lemma=`par` because no real
Italian lemma exists for that fragment. No per-language MWT-override
rule — old or new — operates after POS tagging, so this class of
break is out of reach for the hook. The character-DP realigner in
`align_tokens` also runs at tokenize, before the POS split.

Fixing this properly would require one of:
1. **Lemma-driven content rejection**: detect the signature
   `concat(inner_words) == token_text && head_word.lemma ==
   head_word.text` — the two-signal discriminator that separates
   Defect 6 pseudo-splits from legitimate clitic compounds like
   `dammela → da/dare + me + la` (where head lemma `dare` ≠ head
   text `da`). When it fires, rewrite the merged `%mor` with a
   principled substitute (e.g., surface-form lemma plus a bare
   `verb|…` POS without the junk enclitic), OR refuse to emit a
   `%mor` entry and mark the utterance so the user knows Stanza
   gave up.
2. **Stanza retrain** on conversational Italian where `parla` as
   2sg imperative is marked with its correct `parlare` lemma.
3. **Swap Stanza for CLAN's Italian MOR** as the morphosyntax
   engine for Italian — its lexicon handles these imperative forms
   correctly.

### BA3 mitigation (ACTIVE)

BA3 carries a per-language reconciler
in `crates/talkbank-transform/src/morphosyntax/lang_it.rs` that collapses
the known Defect 6 mis-splits back to a single `%mor` entry with
overridden POS/lemma/features. The `IT_MIS_SPLIT_OVERRIDES`
allowlist covers `parla → verb|parlare`,
`arancione → noun|arancione`, `piccolo → adj|piccolo`,
`gomitolo → noun|gomitolo`, `divano → noun|divano`. The reconciler
fires inside `map_ud_sentence`'s `UdId::Range` branch and records
the collapsed range so `build_gra_and_validate` emits a single
`%gra` relation. See the
[Italian](languages/italian.md) chapter §"Reconciler for Defect
6 / 7" for the full allowlist, constraints, and the three-layer
test strategy (lang_it.rs unit tests + synthetic UD integration
tests + end-to-end morphotag golden).

The UD-level xfail probes below remain pinned — they measure
Stanza's raw output, which the reconciler does not change. The
reconciler operates downstream of Stanza and only affects what
lands in the CHAT `%mor` tier.

### Tests

* **Pinned UD-level observations (xfail):** in
  `batchalign/tests/investigations/test_stanza_mwt_probe_matrix.py`:
  - `test_stanza_mwt_probe_with_postprocessor[ita__dell_opera_in_context]`
  - `[ita__parla_imperative_forte]`
  - `[ita__parla_imperative_piu_forte]`

  Each asserts that Stanza produces as many UD words as CHAT words
  and xfails because Stanza produces one more (the spurious `par + la`
  split at UD level). These pins are **Stanza-behavior observations**,
  NOT injection-gate failure indicators — the actual `%mor` tier emits
  one item per CHAT word for each of these inputs because Stage 3
  collapses the MWT Range. The pins exist so a Stanza upgrade that
  fixes the UD-level anomaly (or a BA3 content-rejection rule that
  intercepts the junk before Stage 3) surfaces as XPASS.
* **Free-tokenize twin (no postprocessor):** the matrix also runs
  each case through a plain Stanza pipeline without BA3's
  tokenizer-postprocessor context — confirming the split is native
  Stanza behavior, not an artifact of our realignment hook.
* **Content-quality probes are NOT yet defined.** There is no
  automated assertion today that `%mor` content for `parla forte`
  matches the correct `verb|parlare-Imp-S2 adj|forte-S1`. Adding
  such a probe requires either a manually-curated expected-`%mor`
  fixture per case or an oracle (e.g., CLAN's Italian MOR output).
  Flagged as a gap.

### Re-evaluation criteria (when Stanza upgrades)

1. Re-run the xfail test. If it starts passing unexpectedly, Stanza
   has fixed the POS-layer split — remove the xfail mark and update
   this entry to a resolved state.
2. If the split shifts to a different surface form (e.g., stops on
   `parla` but starts on `guarda → guard+a`), extend the probe
   case list and re-document.

### Related

Other Italian verbs that share the surface-ending risk (`-la`, `-lo`,
`-le`, `-li`, `-mi`, `-ti`, `-ci`, `-vi`, `-si`, `-ne`) have not yet
been systematically probed. See the
[Italian](languages/italian.md) chapter for the broader audit
context and the full list of examined constructions.

### Scope evidence

A corpus-wide audit of committed `%mor` content
(`scripts/analysis/audit_italian_mor_content.py`, pointed via
`--root` or `$TB_DATA_JSON` at a pre-parsed JSON snapshot of the
TalkBank CHAT corpora) counts **65 Defect-6 hits across 417 Italian
files and 15 distinct surface forms**. The top
surfaces are `parla` (15), `arancione` (13), `piccolo` (10),
`seggiola`/`piccola`/`divano`/`trottola` (3–4 each), and a long
tail of single-file occurrences. Re-run the audit to measure the
delta after any mitigation:

```bash
uv run python scripts/analysis/audit_italian_mor_content.py \
  --jsonl /tmp/italian_defects.jsonl
```

A narrower probe against an ita-only main-tier scan found 73
occurrences of `parla` specifically across 43 files. Probes in
`batchalign/tests/investigations/_cases/italian.py` pin the verb
subclass (`parla_imperative_forte`, `parla_imperative_piu_forte`,
`dell_opera_in_context`) plus representative noun/adjective hits
(`arancione_noun_bogus_verb`, `piccolo_adj_bogus_verb`) as UD-level
Stanza-behavior observations. The minimal end-to-end
`batchalign3 morphotag` run characterizes what `%mor` actually ships
downstream:

| Input | Stanza UD structure (after MWT expansion) | Actual `%mor` emitted | Linguistic correctness |
|-------|--------------------------------------------|----------------------|------------------------|
| `parla forte` | `par/par/VERB + la/la/PRON + forte/forte/ADJ` wrapped as Token(1,2)+Token(3,) | `verb|par-Inf-S~pron|la-Prs-S3 adj|forte-S1` | **Wrong** — fake lemma `par`, spurious clitic |
| `parla più forte` | `par/par/VERB + la/la/PRON + più/più/ADV + forte/forte/ADJ` | `verb|par-Inf-S~pron|la-Prs-S3 adv|più adj|forte-S1` (same junk) | **Wrong** — same shape |
| `parla dell'opera nuova` | `par/par + la/la + dell'/(di+l') + opera + nuova` | compound `verb|par-...~pron|la-...` for `parla` | **Wrong** — same shape |
| `la storia parla di un bambino` | `la/(il+i) + storia + parla/parlare/VERB + di + un + bambino` | `det|il-Masc-Def-Art-Sing~det|il-Masc-Def-Art-Plur noun|storia-Fem verb|parlare-Fin-Ind-Pres-S3 adp|di det|uno-Masc-Ind-Art-Sing noun|bambino-Masc` | **Partially wrong** — `la` emitted as masc-sing+masc-plur clitic (Defect 7); `parla` mid-sentence correct |
| `dammela` | `da/dare/VERB + me/me/PRON + la/la/PRON` wrapped as Token(1,3) | `verb|dare-Inf-Ind-Imp-S2~pron|me-Prs-S1~pron|la-Prs-S3` | **Correct** — imperative of `dare` with double-clitic stack |
| `per favore dammela` | `per/ADP + favore/NOUN + dammela/dammelo/ADJ` (no MWT expansion mid-sentence) | `adp|per noun|favore-Masc adj|dammelo-S1` | **Wrong** — different Stanza defect: mid-sentence compound tagged ADJ, lemma normalized to `dammelo`, no clitic decomposition. Separate from Defect 6. |

Three conclusions, all pipeline-verified:

1. **The `%mor` count invariant is NOT violated by any of these.**
   Stage 3's `assemble_mors` collapses MWT Range components into a
   single compound `%mor` using `~`/`+`, so every CHAT word gets
   exactly one `%mor` item. The
   `mor_count_parity_reference_corpus.rs` test still passes. **Defect 6 is not an injection-gate failure**
   — it's a linguistic-content failure downstream of Stanza's POS
   layer.
2. **`dammela` alone is handled correctly.** Stanza produces the
   right imperative+clitic analysis (`dare` lemma + `me` + `la`
   clitics), Stage 3 assembles the compound `%mor`, and the output
   matches UD convention for Italian clitic compounds. **No bug
   here.**
3. **`dammela` mid-sentence (`per favore dammela`) fails for a
   different reason.** Stanza misclassifies the entire compound as
   ADJ with lemma `dammelo`, skipping MWT expansion entirely. This
   is a separate Italian Stanza defect (Defect 8, mitigated by a
   single-chunk POS/lemma override — see Defect 8 below).

A discriminator between Defect-6-style junk (`parla → par+la` with
lemma=`par`) and a legitimate clitic compound (`dammela → da+me+la`
with lemma=`dare`) is **not needed at the `%mor` injection level**
— the mapper already handles both correctly in terms of count.

The per-language reconciler in `crates/talkbank-transform/src/morphosyntax/lang_it.rs` takes
a different approach than lemma-equality heuristics — it uses a
closed allowlist of known-defective input-token texts (`parla`,
`arancione`, `piccolo`, `gomitolo`, `divano`, Defect 7's `la`) and
overrides only those. The `dammela` regression guard test confirms
genuine verb+clitic compounds remain correctly merged. A
lemma-equality heuristic might have broader coverage but would risk
false positives on legitimate Italian verbs whose lemma happens to
match the surface form. Allowlist-first is safer; corpus-sweep
evidence can extend it case-by-case.

---

## Defect 7: Italian sentence-initial article `la` gets junk MWT expansion (`il + i`)

<a id="stanza-it-la-sentence-initial-split"></a>

* **Stable slug:** ``stanza-it-la-sentence-initial-split``
* **Stanza version:** 1.11.1 and 1.12.0 (both confirmed; re-verified via MWT probe matrix `xfail` markers held on 2026-05-14)
* **MWT package:** Italian default
* **Failure class:** linguistic-content quality. Stage 3's
  `assemble_mors` collapses the bogus 2-word expansion into a single
  compound `%mor` entry per CHAT word, so the count invariant holds.
  The content is wrong:
  `det|il-Masc-Def-Art-Sing~det|il-Masc-Def-Art-Plur` for a
  feminine-singular article carries wrong lemma, wrong features, and
  wrong number agreement.
* **Construction:** Sentence-initial feminine singular article `la`
  (as in `la storia`, `la casa`) is wrapped by Stanza in an MWT
  Token whose inner words are `il` + `i`, both tagged DET and both
  lemmatized to `il`. This is spurious — `la` is a single-morpheme
  article that should be analyzed as `det|la-Fem-Def-Art-Sing`.

### Input and observed end-to-end `%mor` output

```text
*CHI:	la storia parla di un bambino .
%mor:	det|il-Masc-Def-Art-Sing~det|il-Masc-Def-Art-Plur noun|storia-Fem
        verb|parlare-Fin-Ind-Pres-S3 adp|di det|uno-Masc-Ind-Art-Sing
        noun|bambino-Masc .
%gra:	1|3|DET 2|3|DET 3|4|NSUBJ 4|0|ROOT 5|7|CASE 6|7|DET 7|4|OBL 8|4|PUNCT
```

The first `%mor` chunk is ONE item for the CHAT word `la` (Range
collapse works), but the linguistic content is two masculine-article
readings — neither of which matches the input's feminine-singular
`la`. The rest of the utterance is linguistically correct
(`parla` mid-sentence gets its proper `parlare-Fin-Ind-Pres-S3`
analysis — unrelated to Defect 6).

### Correct output

```text
*CHI:	la storia parla di un bambino .
%mor:	det|la-Fem-Def-Art-Sing noun|storia-Fem verb|parlare-Fin-Ind-Pres-S3
        adp|di det|uno-Masc-Ind-Art-Sing noun|bambino-Masc .
```

One item per CHAT word with the right feminine-singular analysis on
`la`. Stanza's Italian MWT model does not produce this today.

### Scope — position sensitivity not yet characterized

The spurious expansion has only been observed at sentence-initial
position in the current probe matrix. It is not yet known whether
mid-sentence `la` (e.g., `vedo la storia`) also triggers it. The
MWT probe in `scripts/analysis/probe_stanza_italian_mwt_metadata.py`
has a hook for extending the case list; do that before proposing a
fix.

### Why no tokenizer hack rescues it

Same structural limitation as Defect 6: the expansion is produced by
Stanza's MWT processor, which runs after tokenize. The
`tokenize_postprocessor` hook sees a single token `la` and cannot
block the downstream MWT rule from firing.

### BA3 mitigation (ACTIVE)

The per-language reconciler introduced for Defect 6 also handles this
case. The `la` entry in `IT_MIS_SPLIT_OVERRIDES` in
`crates/talkbank-transform/src/morphosyntax/lang_it.rs` catches the
Range parent whose text is `la`, regardless of the specific
component texts Stanza emits. The reconciler replaces the junk
`det|il-Masc-Def-Art-Sing~det|il-Masc-Def-Art-Plur` with a
single `det|il-Fem-Def-Art-Sing` (or equivalent) `%mor` entry.

Same reconciler architecture as Defect 6, different allowlist
entry. The two defects are orthogonal at the detection level
(Defect 6's components concatenate back to the input text;
Defect 7's do not, e.g. `il + i ≠ la`) but the reconciler's
range-parent-text lookup handles both uniformly — it doesn't
need to distinguish defect families because the allowlist key
IS the input token, not the component signature.

See [Italian](languages/italian.md) §"Reconciler for Defect 6 / 7"
for the full allowlist.

### Tests

* **Pinned UD-level observation (xfail):**
  `batchalign/tests/investigations/test_stanza_mwt_probe_matrix.py::test_stanza_mwt_probe_with_postprocessor[ita__parla_3sg_storia_context]`
  asserts that Stanza produces exactly 6 UD words for the 6-word CHAT
  input. It fails because Stanza produces 7 (spurious `la → il + i`
  split at UD level). The xfail is a **Stanza-behavior pin**, not an
  injection-gate failure indicator — the actual `%mor` tier emits
  correctly with 6 items because Stage 3 collapses the Range. The
  pin exists so a Stanza upgrade that fixes the UD-level anomaly
  surfaces as XPASS; when that happens, `%mor` content will also
  improve automatically.

### Re-evaluation criteria (when Stanza upgrades)

1. Re-run the xfail test. If it flips to unexpected pass, Stanza has
   fixed the sentence-initial expansion — remove the xfail mark and
   update this entry to a resolved state.
2. If the expansion shifts to a different surface form (e.g., `le` or
   `li` also start expanding), extend the probe case list and
   re-document.

### Related

Shares the "post-tokenize architectural gap" with Defect 6. A single
post-POS/MWT reassembly pass could in principle address both, but the
detection rule is different: Defect 6 concatenates back to the input
token, Defect 7 does not. See the
[Italian](languages/italian.md) chapter and Defect 6 for the full
audit context.

---

## Defect 8: Italian imperative+enclitic compounds mid-sentence mis-tagged as ADJ

<a id="stanza-it-compound-imperative-mid-sentence-adj"></a>

* **Stable slug:** ``stanza-it-compound-imperative-mid-sentence-adj``
* **Stanza version:** 1.11.1 and 1.12.0 (both confirmed; re-verified via MWT probe matrix `xfail` markers held on 2026-05-14)
* **MWT package:** Italian default
* **Failure class:** linguistic-content quality. Stanza tokenizes
  the compound correctly (one UD word) but mis-classifies its POS
  and normalizes the lemma.

### Construction

Italian imperative+enclitic compounds (`dammela`, `dammelo`,
similar) are correctly handled when they appear alone — Stanza's
MWT processor fires and emits a three-word expansion
(`verb|dare~pron|me~pron|la`). In **mid-sentence** position
(e.g., `per favore dammela`), Stanza's MWT processor does NOT
fire. The compound surfaces as a single UD word tagged
`ADJ` with lemma normalized to `dammelo` (masculine-singular
reading of the final clitic). The resulting `%mor` ships as
`adj|dammelo-S1` instead of the correct `verb|dare-Imp-S2` or
the decomposed `verb|dare~pron|me~pron|la`.

### Input and observed output

```text
Stanza input: ["per", "favore", "dammela"]
Stanza UD output (mid-sentence): [
    (per, ADP, lemma=per),
    (favore, NOUN, lemma=favore),
    (dammela, ADJ, lemma=dammelo),  ← mis-tagged
]
%mor without reconciler: adp|per noun|favore-Masc adj|dammelo-S1
```

### BA3 mitigation (ACTIVE)

`crates/talkbank-transform/src/morphosyntax/lang_it.rs` carries a second
allowlist `IT_COMPOUND_IMPERATIVES` separate from the
Defect-6/7 `IT_MIS_SPLIT_OVERRIDES`. Entries name the surface
form, the correct verb lemma, and the correct feats. Current
entries: `dammela → dare`, `dammelo → dare`. The reconciler fires
inside `map_ud_sentence`'s `UdId::Single` branch, gated on
`upos == ADJ` + text match against the allowlist.

**Scope**: the mitigation emits a **single-chunk** `Mor`
(`verb|dare-Imp-S2`) rather than decomposing the compound into
verb + clitic post-clitics. This is a scope trade-off — a multi-
chunk emission from a single UdWord would require extending
`build_gra_and_validate`'s chunk-counting logic (currently
assumes `UdId::Single` → exactly one chunk). The single-chunk
fix captures the correct POS and verb lemma, which is a
substantial improvement over `adj|dammelo`. Multi-chunk
decomposition is a future enhancement.

**Extension**: new compound imperatives observed in corpus data
are added as one row to `IT_COMPOUND_IMPERATIVES` plus a
regression test in `morphosyntax/tests.rs`.

### Tests

- **Synthetic-UD unit tests** in
  `crates/batchalign/src/chat_ops/nlp/mapping/tests/italian_defects.rs`:
  - `test_italian_defect8_dammela_mid_sentence_becomes_verb`
  - `test_italian_defect8_dammelo_mid_sentence_becomes_verb`
  - `test_italian_defect8_genuine_adj_stays_adj` (control)
- **Allowlist unit tests** in
  `crates/talkbank-transform/src/morphosyntax/lang_it.rs`.
- **End-to-end golden** in
  `batchalign/tests/pipelines/morphosyntax/test_italian_defect6_end_to_end.py::test_dammela_mid_sentence_becomes_verb`
  — runs `batchalign3 morphotag` on a CHAT fixture whose
  `@Languages:` header declares `ita`, content `per favore
  dammela`, and asserts the output `%mor` carries `dare`, not
  `adj|dammelo`. (Morphotag has no `--lang` flag; language
  comes from each file's `@Languages:` header.)

### Re-evaluation criteria

If a Stanza upgrade produces a correct multi-chunk MWT expansion
for mid-sentence compound imperatives (i.e., Stanza emits a Range
for `dammela` wherever it appears), the Defect 8 allowlist entries
become redundant. Remove them and let the Defect-6/7 Range branch
handle the case uniformly. Tracked by the unit-test RED signal
when the reconciler is disabled and Stanza is re-observed.

---

## Process for adding entries

When a new Stanza limitation is discovered:

1. Write a permanent test capturing what Stanza produces for the
   failing input, with the Stanza version recorded in a comment or
   docstring. The test asserts the CORRECT behavior and is therefore
   RED under the current Stanza.
2. Add a section here following the format above.
3. Implement a principled BA3 mitigation (or, if no mitigation is
   feasible yet, document the issue and leave the test RED as a
   known-defect marker with a clear comment).
4. Link the test, the mitigation code, and this registry together so
   future contributors can trace all three in one step.

## Process for re-evaluating on Stanza upgrade

1. Disable the BA3 mitigations (comment out the override entry points
   or run with a feature flag).
2. Run all tests in this document's "Tests" sections.
3. For each test that flips RED→GREEN without the mitigation, Stanza
   has improved. Remove or narrow the corresponding BA3 override,
   update this document, and re-enable normal CI.
4. For tests that remain RED, the mitigation is still load-bearing;
   leave it in place.
