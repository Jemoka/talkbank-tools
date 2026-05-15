"""English decision-probe cases — v2 (token-centric + n-to-m).

Adjudication status (2026-04-23)
---------------------------------
The v2 golden run (see
``docs/investigations/2026-04-23-stanza-decision-probe-findings.md``)
produced clean observations. Adjudicator answered Q-A (ship transcribe
rules on 'no observed Stanza regression') with YES. The adjudication
policy applied here:

* **POST_NEUTRAL locked** — every "both match" case. The rule is a
  safe orthographic change; no Stanza effect. Shipping is approved.
* **POST_STRICTLY_WORSE locked** — DECIMAL_CONTROL cases. The rule
  MUST NOT fire on decimals; the comparator flags the semantic
  loss via text-gold.
* **OBSERVE_ONLY retained** — cases where gold calibration (Q-B,
  queued for adjudication) would change the verdict, and the
  `letter_i_in_alphabet` probe where Stanza-is-wrong-either-way
  means the rule's correctness lives outside the comparator's
  scope.

v2 design
---------
v1 addressed affected tokens by integer index against pre-tokenized
input and carried a flat gold POS tuple. That broke on MWT
expansion and could not express n-to-m. v2 addresses pre-MWT tokens
via :class:`StanzaTokenOutput`, carries per-side :class:`Gold`, and
expresses alignment via :class:`TokenMapping` records.

Gold conventions
----------------
Gold reflects Universal Dependencies EWT best-effort linguistic
judgment. Some entries disagree with Stanza's observed output; those
cases stay OBSERVE_ONLY pending Q-B (correct gold to Stanza vs hold
UD-EWT convention).
"""

from __future__ import annotations

from .._decision_probe_types import (
    CandidateClass,
    DecisionOutcome,
    DecisionProbeCase,
    Gold,
    TokenMapping,
)


def _case(
    *,
    label: str,
    candidate_class: CandidateClass,
    utterance_prose: str,
    pre_words: tuple[str, ...],
    post_words: tuple[str, ...],
    mappings: tuple[TokenMapping, ...],
    rationale: str,
    expected_outcome: DecisionOutcome = DecisionOutcome.OBSERVE_ONLY,
) -> DecisionProbeCase:
    """Case factory. Default ``OBSERVE_ONLY`` is the safe state for
    new cases; adjudicated cases pass an explicit ``expected_outcome``
    (the locked verdict)."""
    return DecisionProbeCase(
        label=label,
        candidate_class=candidate_class,
        utterance_prose=utterance_prose,
        pre_words=pre_words,
        post_words=post_words,
        affected_mappings=mappings,
        expected_outcome=expected_outcome,
        rationale=rationale,
    )


def _one_to_one(
    pre_idx: int,
    post_idx: int,
    pre_upos: tuple[str, ...],
    post_upos: tuple[str, ...],
) -> TokenMapping:
    """Shorthand for a 1-to-1 mapping with symmetric UPOS gold."""
    return TokenMapping(
        pre_token_indices=(pre_idx,),
        post_token_indices=(post_idx,),
        gold=Gold(pre_upos=pre_upos, post_upos=post_upos),
    )


# Rationale suffix recording the adjudication decision. Attached to
# every locked case so the per-case rationale carries its own
# provenance (you can read one case file and see why it's locked).
_LOCK_Q_A = " [locked 2026-04-23, Adjudicator Q-A=ship-on-neutrality]"
_LOCK_CONTROL = (
    " [locked 2026-04-23, Adjudicator Q-A: control must fire POST_STRICTLY_WORSE]"
)
_LOCK_Q_B = (
    " [locked 2026-04-23, Q-B adjudication: Stanza POS > Claude gold; "
    "per-side Stanza-calibrated gold means the probe now sentinels "
    "Stanza behavior rather than asserting a transformation verdict]"
)


# ─── TITLE_PERIOD: Dr. / Mr. / Mrs. / Prof. — LOCKED POST_NEUTRAL ────
_TITLE_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="dr_before_name",
        candidate_class=CandidateClass.TITLE_PERIOD,
        utterance_prose="Dr. Matthews is here.",
        pre_words=("Dr.", "Matthews", "is", "here"),
        post_words=("Dr", "Matthews", "is", "here"),
        mappings=(_one_to_one(0, 0, ("PROPN",), ("PROPN",)),),
        rationale="The approved Dr. → Dr rule. Stanza PROPN both sides." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="mr_before_name",
        candidate_class=CandidateClass.TITLE_PERIOD,
        utterance_prose="Mr. Smith arrived.",
        pre_words=("Mr.", "Smith", "arrived"),
        post_words=("Mr", "Smith", "arrived"),
        mappings=(_one_to_one(0, 0, ("PROPN",), ("PROPN",)),),
        rationale="Extends Dr. probe to Mr. Stanza PROPN both sides." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="mrs_before_name",
        candidate_class=CandidateClass.TITLE_PERIOD,
        utterance_prose="Mrs. Jones left.",
        pre_words=("Mrs.", "Jones", "left"),
        post_words=("Mrs", "Jones", "left"),
        mappings=(_one_to_one(0, 0, ("PROPN",), ("PROPN",)),),
        rationale="Mrs. has no expansion ambiguity." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="prof_before_name",
        candidate_class=CandidateClass.TITLE_PERIOD,
        utterance_prose="Prof. Lee teaches here.",
        pre_words=("Prof.", "Lee", "teaches", "here"),
        post_words=("Prof", "Lee", "teaches", "here"),
        mappings=(_one_to_one(0, 0, ("PROPN",), ("PROPN",)),),
        rationale="Prof alone can be common noun; name context holds this ambiguous."
        + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# Alternative rule shape (split vs strip). LOCKED POST_NEUTRAL — the
# split rule also preserves PROPN on both sides; either rule is safe.
# Keeping the probe locked means if someone ever proposes switching
# from strip to split, the probe will confirm the new rule is also
# Stanza-neutral.
_TITLE_SPLIT_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="dr_period_split_alternative",
        candidate_class=CandidateClass.TITLE_PERIOD,
        utterance_prose="Dr. Matthews (alternate rule: Dr . Matthews)",
        pre_words=("Dr.", "Matthews"),
        post_words=("Dr", ".", "Matthews"),
        mappings=(
            TokenMapping(
                pre_token_indices=(0,),
                post_token_indices=(0, 1),
                gold=Gold(pre_upos=("PROPN",), post_upos=("PROPN", "PUNCT")),
            ),
        ),
        rationale=(
            "Alternative split rule also produces expected POS on both "
            "sides; 1-to-2 n-to-m mapping works." + _LOCK_Q_A
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── PLACE_PERIOD — LOCKED POST_NEUTRAL ──────────────────────────────
_PLACE_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="st_as_street",
        candidate_class=CandidateClass.PLACE_PERIOD,
        utterance_prose="Main St. runs north.",
        pre_words=("Main", "St.", "runs", "north"),
        post_words=("Main", "St", "runs", "north"),
        mappings=(_one_to_one(1, 1, ("PROPN",), ("PROPN",)),),
        rationale="St. / St ambiguity with Saint; context fixed." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="mt_as_mount",
        candidate_class=CandidateClass.PLACE_PERIOD,
        utterance_prose="Mt. Everest is tall.",
        pre_words=("Mt.", "Everest", "is", "tall"),
        post_words=("Mt", "Everest", "is", "tall"),
        mappings=(_one_to_one(0, 0, ("PROPN",), ("PROPN",)),),
        rationale="Place abbreviation + proper name." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="ave_as_avenue",
        candidate_class=CandidateClass.PLACE_PERIOD,
        utterance_prose="Fifth Ave. is busy.",
        pre_words=("Fifth", "Ave.", "is", "busy"),
        post_words=("Fifth", "Ave", "is", "busy"),
        mappings=(_one_to_one(1, 1, ("PROPN",), ("PROPN",)),),
        rationale="Three-letter place abbreviation." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── TIME_PERIOD — LOCKED POST_NEUTRAL ───────────────────────────────
_TIME_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="am_after_number",
        candidate_class=CandidateClass.TIME_PERIOD,
        utterance_prose="We met at nine a.m.",
        pre_words=("We", "met", "at", "nine", "a.m."),
        post_words=("We", "met", "at", "nine", "am"),
        mappings=(_one_to_one(4, 4, ("NOUN",), ("NOUN",)),),
        rationale=(
            "Worried about am/verb collision; Stanza disambiguates "
            "correctly (both NOUN)." + _LOCK_Q_A
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="pm_after_number",
        candidate_class=CandidateClass.TIME_PERIOD,
        utterance_prose="Dinner starts at six p.m.",
        pre_words=("Dinner", "starts", "at", "six", "p.m."),
        post_words=("Dinner", "starts", "at", "six", "pm"),
        mappings=(_one_to_one(4, 4, ("NOUN",), ("NOUN",)),),
        rationale="pm has no verb collision; control sibling of am." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── TECHNICAL_ABBREV — LOCKED POST_NEUTRAL (Q-B adjudication) ───────
# Adjudication (2026-04-23): "I'm happy to see the periods stripped from
# these, but I take the Stanza POS as better than the Claude." Gold
# is now Stanza-calibrated per-side. Each probe matches Stanza's
# current observed tag on its own side → POST_NEUTRAL. Future
# Stanza drift will surface as a test failure for re-adjudication.
_TECHNICAL_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="etc_trailing",
        candidate_class=CandidateClass.TECHNICAL_ABBREV,
        utterance_prose="apples, oranges, etc.",
        pre_words=("apples", "oranges", "etc."),
        post_words=("apples", "oranges", "etc"),
        mappings=(_one_to_one(2, 2, ("NOUN",), ("NOUN",)),),
        rationale=(
            "Latin et cetera. Stanza tags both as NOUN; period "
            "stripping approved via Q-B." + _LOCK_Q_B
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="eg_inline",
        candidate_class=CandidateClass.TECHNICAL_ABBREV,
        utterance_prose="fruits, e.g., apples",
        pre_words=("fruits", "e.g.", "apples"),
        post_words=("fruits", "eg", "apples"),
        mappings=(
            TokenMapping(
                pre_token_indices=(1,),
                post_token_indices=(1,),
                gold=Gold(pre_upos=("ADV",), post_upos=("NOUN",)),
            ),
        ),
        rationale=(
            "Stanza: pre ADV, post NOUN. Different-shaped gold per "
            "side is legal under v2; each matches its own Stanza "
            "observation." + _LOCK_Q_B
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="ie_inline",
        candidate_class=CandidateClass.TECHNICAL_ABBREV,
        utterance_prose="the capital, i.e., Paris",
        pre_words=("the", "capital", "i.e.", "Paris"),
        post_words=("the", "capital", "ie", "Paris"),
        mappings=(
            TokenMapping(
                pre_token_indices=(2,),
                post_token_indices=(2,),
                gold=Gold(pre_upos=("ADV",), post_upos=("ADP",)),
            ),
        ),
        rationale=(
            "Stanza: pre ADV, post ADP. Same per-side pattern as "
            "eg_inline; both match their respective golds." + _LOCK_Q_B
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── INITIALISM_PERIOD — LOCKED POST_NEUTRAL ─────────────────────────
_INITIALISM_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="us_with_periods",
        candidate_class=CandidateClass.INITIALISM_PERIOD,
        utterance_prose="the U.S. government",
        pre_words=("the", "U.S.", "government"),
        post_words=("the", "US", "government"),
        mappings=(_one_to_one(1, 1, ("PROPN",), ("PROPN",)),),
        rationale="US vs us pronoun is case-disambiguated; caseful still PROPN."
        + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="jfk_initials",
        candidate_class=CandidateClass.INITIALISM_PERIOD,
        utterance_prose="J.F.K. was president.",
        pre_words=("J.F.K.", "was", "president"),
        post_words=("JFK", "was", "president"),
        mappings=(_one_to_one(0, 0, ("PROPN",), ("PROPN",)),),
        rationale="Three-letter personal initialism; JFK is canonical." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── DEGREE_PERIOD — mixed adjudication ──────────────────────────────
_DEGREE_CASES: tuple[DecisionProbeCase, ...] = (
    # LOCKED POST_NEUTRAL: Stanza agrees with NOUN gold on both sides.
    _case(
        label="phd_trailing",
        candidate_class=CandidateClass.DEGREE_PERIOD,
        utterance_prose="She has a Ph.D.",
        pre_words=("She", "has", "a", "Ph.D."),
        post_words=("She", "has", "a", "PhD"),
        mappings=(_one_to_one(3, 3, ("NOUN",), ("NOUN",)),),
        rationale="Degree as object-nominal; PhD is modern standard. Stanza NOUN both."
        + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    # LOCKED POST_NEUTRAL: Q-B — Stanza's PROPN wins over
    # UD-EWT NOUN. Gold recalibrated.
    _case(
        label="md_trailing",
        candidate_class=CandidateClass.DEGREE_PERIOD,
        utterance_prose="He is an M.D.",
        pre_words=("He", "is", "an", "M.D."),
        post_words=("He", "is", "an", "MD"),
        mappings=(_one_to_one(3, 3, ("PROPN",), ("PROPN",)),),
        rationale=(
            "Stanza tags M.D./MD as PROPN (treats as proper "
            "abbreviation). Q-B: use Stanza POS over UD-EWT "
            "NOUN gold." + _LOCK_Q_B
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── DECIMAL_CONTROL — LOCKED POST_STRICTLY_WORSE ────────────────────
# These are control probes: they prove the period-strip rule MUST NOT
# fire on decimals. The comparator's text-gold catches `3.14` → `3`
# as a semantic regression — exactly what we want to see.
_DECIMAL_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="pi_decimal",
        candidate_class=CandidateClass.DECIMAL_CONTROL,
        utterance_prose="pi is about 3.14",
        pre_words=("pi", "is", "about", "3.14"),
        post_words=("pi", "is", "about", "3"),
        mappings=(
            TokenMapping(
                pre_token_indices=(3,),
                post_token_indices=(3,),
                gold=Gold(
                    pre_upos=("NUM",),
                    post_upos=("NUM",),
                    pre_text=("3.14",),
                    post_text=("3.14",),
                ),
            ),
        ),
        rationale=(
            "Control: text-gold catches semantic loss when the strip "
            "rule fires on a decimal. The rule must be gated so this "
            "case never materializes in production." + _LOCK_CONTROL
        ),
        expected_outcome=DecisionOutcome.POST_STRICTLY_WORSE,
    ),
    _case(
        label="price_decimal",
        candidate_class=CandidateClass.DECIMAL_CONTROL,
        utterance_prose="that is 2.50 a pound",
        pre_words=("that", "is", "2.50", "a", "pound"),
        post_words=("that", "is", "2", "a", "pound"),
        mappings=(
            TokenMapping(
                pre_token_indices=(2,),
                post_token_indices=(2,),
                gold=Gold(
                    pre_upos=("NUM",),
                    post_upos=("NUM",),
                    pre_text=("2.50",),
                    post_text=("2.50",),
                ),
            ),
        ),
        rationale="Second decimal control; trailing zero distinguishes from pi."
        + _LOCK_CONTROL,
        expected_outcome=DecisionOutcome.POST_STRICTLY_WORSE,
    ),
)

# ─── SENTENCE_PERIOD — LOCKED POST_NEUTRAL (1-to-0 deletion) ─────────
_SENTENCE_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="final_period_deletion",
        candidate_class=CandidateClass.SENTENCE_PERIOD,
        utterance_prose="I saw him.",
        pre_words=("I", "saw", "him", "."),
        post_words=("I", "saw", "him"),
        mappings=(
            TokenMapping(
                pre_token_indices=(3,),
                post_token_indices=(),
                gold=Gold(pre_upos=("PUNCT",)),
            ),
        ),
        rationale=(
            "1-to-0 deletion mapping: pre-period matches gold PUNCT, "
            "post has no corresponding token (clean deletion)." + _LOCK_Q_A
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── ENGLISH_PRONOUN_I — LOCKED POST_NEUTRAL ─────────────────────────
_PRONOUN_I_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="i_think",
        candidate_class=CandidateClass.ENGLISH_PRONOUN_I,
        utterance_prose="i think so.",
        pre_words=("i", "think", "so"),
        post_words=("I", "think", "so"),
        mappings=(_one_to_one(0, 0, ("PRON",), ("PRON",)),),
        rationale=(
            "The approved highest-priority rule. Stanza tags bare i as PRON "
            "already; I-cap is orthographic, not morphotag-driven." + _LOCK_Q_A
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="i_midsentence",
        candidate_class=CandidateClass.ENGLISH_PRONOUN_I,
        utterance_prose="yes i went home.",
        pre_words=("yes", "i", "went", "home"),
        post_words=("yes", "I", "went", "home"),
        mappings=(_one_to_one(1, 1, ("PRON",), ("PRON",)),),
        rationale="Pronoun i not in utterance-initial position." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── LETTER_I_CONTROL — OBSERVE_ONLY (Stanza wrong both ways) ────────
# Stanza tags both `i` and `I` as PRON in the "the letter X" context
# because the CHAT @l marker is stripped before Stanza sees the word.
# The comparator correctly reports "neither matches" in notes but
# aggregates to POST_NEUTRAL. This probe documents that the rule's
# correctness depends on BA3 RESPECTING @l in CHAT before Stanza is
# invoked — a pipeline-level constraint outside the comparator's
# scope. Verdict-lock waits until the rule implementation handles
# the @l marker, at which point a new probe can test the pipeline
# directly.
_LETTER_I_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="letter_i_in_alphabet",
        candidate_class=CandidateClass.LETTER_I_CONTROL,
        utterance_prose="the letter i@l comes after h@l.",
        pre_words=("the", "letter", "i", "comes", "after", "h"),
        post_words=("the", "letter", "I", "comes", "after", "h"),
        mappings=(_one_to_one(2, 2, ("NOUN",), ("NOUN",)),),
        rationale=(
            "Stanza tags bare i/I as PRON in either form (wrong for "
            "the letter-name sense). The rule must gate on CHAT @l "
            "before Stanza runs; probe cannot adjudicate that "
            "pipeline concern alone."
        ),
    ),
)

# ─── I_CONTRACTION — LOCKED POST_NEUTRAL ─────────────────────────────
# v1's "MWT indexing bug" cases; v2 token-centric addressing pins
# both expanded UD words on each side. Stanza MWT-expands both
# lowercase and uppercase I-contractions → both match gold
# (PRON, AUX).
_I_CONTRACTION_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="ill_contraction",
        candidate_class=CandidateClass.I_CONTRACTION,
        utterance_prose="i'll go now.",
        pre_words=("i'll", "go", "now"),
        post_words=("I'll", "go", "now"),
        mappings=(
            TokenMapping(
                pre_token_indices=(0,),
                post_token_indices=(0,),
                gold=Gold(pre_upos=("PRON", "AUX"), post_upos=("PRON", "AUX")),
            ),
        ),
        rationale=(
            "MWT-aware gold pins both expanded UD words; Stanza "
            "correctly expands both lowercase and uppercase forms." + _LOCK_Q_A
        ),
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="im_contraction",
        candidate_class=CandidateClass.I_CONTRACTION,
        utterance_prose="i'm fine.",
        pre_words=("i'm", "fine"),
        post_words=("I'm", "fine"),
        mappings=(
            TokenMapping(
                pre_token_indices=(0,),
                post_token_indices=(0,),
                gold=Gold(pre_upos=("PRON", "AUX"), post_upos=("PRON", "AUX")),
            ),
        ),
        rationale="Most frequent I-contraction; MWT expansion works." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="ive_contraction",
        candidate_class=CandidateClass.I_CONTRACTION,
        utterance_prose="i've done it.",
        pre_words=("i've", "done", "it"),
        post_words=("I've", "done", "it"),
        mappings=(
            TokenMapping(
                pre_token_indices=(0,),
                post_token_indices=(0,),
                gold=Gold(pre_upos=("PRON", "AUX"), post_upos=("PRON", "AUX")),
            ),
        ),
        rationale="Third I-contraction; distinct aux from i'll / i'm." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)

# ─── UTTERANCE_INITIAL_CAP — LOCKED POST_NEUTRAL ─────────────────────
_UTTERANCE_INITIAL_CASES: tuple[DecisionProbeCase, ...] = (
    _case(
        label="hello_initial",
        candidate_class=CandidateClass.UTTERANCE_INITIAL_CAP,
        utterance_prose="hello world.",
        pre_words=("hello", "world"),
        post_words=("Hello", "world"),
        mappings=(_one_to_one(0, 0, ("INTJ",), ("INTJ",)),),
        rationale="Utterance-initial greeting; INTJ unchanged by case." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="the_initial",
        candidate_class=CandidateClass.UTTERANCE_INITIAL_CAP,
        utterance_prose="the dog barked.",
        pre_words=("the", "dog", "barked"),
        post_words=("The", "dog", "barked"),
        mappings=(_one_to_one(0, 0, ("DET",), ("DET",)),),
        rationale="Determiner at utterance-start; case-robust." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
    _case(
        label="what_initial_question",
        candidate_class=CandidateClass.UTTERANCE_INITIAL_CAP,
        utterance_prose="what did you do?",
        pre_words=("what", "did", "you", "do"),
        post_words=("What", "did", "you", "do"),
        mappings=(_one_to_one(0, 0, ("PRON",), ("PRON",)),),
        rationale="Interrogative pronoun at utterance-start." + _LOCK_Q_A,
        expected_outcome=DecisionOutcome.POST_NEUTRAL,
    ),
)


# Aggregate — order is descriptive, not semantic.
CASES: tuple[DecisionProbeCase, ...] = (
    *_TITLE_CASES,
    *_TITLE_SPLIT_CASES,
    *_PLACE_CASES,
    *_TIME_CASES,
    *_TECHNICAL_CASES,
    *_INITIALISM_CASES,
    *_DEGREE_CASES,
    *_DECIMAL_CASES,
    *_SENTENCE_CASES,
    *_PRONOUN_I_CASES,
    *_LETTER_I_CASES,
    *_I_CONTRACTION_CASES,
    *_UTTERANCE_INITIAL_CASES,
)
