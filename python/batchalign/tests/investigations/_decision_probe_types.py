"""Typed data model for Stanza normalization-decision probes.

Companion to :mod:`._probe_types` (MWT probes). Where the MWT family
asks "does Stanza tokenize this CHAT word sequence the way we
expect?", this module asks "does a proposed normalization rule X
change Stanza's morphotag output on the affected tokens — and in
which direction?"

Design (v2, 2026-04-23)
-----------------------
v2 replaces v1's per-word integer addressing with token-centric
addressing plus n-to-m :class:`TokenMapping` records. Motivation:
the first golden run (see
``docs/investigations/2026-04-23-stanza-decision-probe-findings.md``)
surfaced three limits of v1:

* **MWT expansion breaks integer indexing.** Stanza tokenizes
  ``i'll`` as one input token but expands to two UD words (``i``,
  ``will``). v1 addressed UD words by their position in the
  flattened ``doc.sentences[*].words`` list but case authors wrote
  indices against the input word list. For any case that hit an
  MWT-triggering word, the two diverged silently.
* **POS-only comparator misses semantic loss.** v1 compared only
  UPOS, so ``3.14`` → ``3`` reported NEUTRAL (both NUM) even though
  the number's value changed. v2 supports text-gold alongside
  POS-gold so semantic regressions surface.
* **No n-to-m alignment.** v1 required ``len(pre_indices) ==
  len(post_indices)``. Period splits (``Dr.`` → ``Dr`` + ``.``),
  period deletions (``saw him .`` → ``saw him``), and number
  expansions (``23`` → ``twenty three``) all need different token
  counts on each side.

v2 fixes all three by (1) addressing tokens pre-MWT with
:class:`StanzaTokenOutput` carrying the post-MWT expansion, (2)
gold-per-side via :class:`Gold` so mismatched shapes are legal,
(3) mappings as first-class records instead of paired index tuples.

Purity
------
:func:`compare_stanza_outputs` is pure — given the synthesized
:class:`StanzaTokenOutput` inputs it produces a deterministic
:class:`DecisionComparison` without loading Stanza. Phase-1 tests
exercise this; the runner (Phase 2) constructs token outputs from
real Stanza Documents and feeds them to the same comparator.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from ._probe_types import XfailMark


# ─── Outcome + candidate-class enums (unchanged from v1) ─────────────


class DecisionOutcome(Enum):
    """The relationship we expect to observe between Stanza's
    morphotag output on the pre-form and the post-form of a
    normalization-decision probe."""

    POST_STRICTLY_BETTER = "post_strictly_better"
    """Post matches gold on at least one affected mapping where pre
    does not, and no mapping regresses. Evidence for shipping."""

    POST_NEUTRAL = "post_neutral"
    """Every affected mapping has pre and post matching gold
    equivalently (both match, or both fail to match in the same
    way). The transformation is a no-op at the observed level."""

    POST_STRICTLY_WORSE = "post_strictly_worse"
    """At least one mapping regresses and none improve. Evidence
    against shipping, or a flag to document a trade-off."""

    MIXED = "mixed"
    """At least one mapping improves and at least one regresses.
    Requires linguistic-judgment adjudication."""

    OBSERVE_ONLY = "observe_only"
    """No assertion. Used while the case's linguistic status is
    itself under investigation — the probe pins current Stanza
    behavior so a future contributor can lock a verdict."""


class CandidateClass(Enum):
    """Classifies which normalization rule a decision probe speaks to."""

    TITLE_PERIOD = "title_period"
    PLACE_PERIOD = "place_period"
    TIME_PERIOD = "time_period"
    TECHNICAL_ABBREV = "technical_abbrev"
    INITIALISM_PERIOD = "initialism_period"
    DEGREE_PERIOD = "degree_period"
    DECIMAL_CONTROL = "decimal_control"
    SENTENCE_PERIOD = "sentence_period"
    ENGLISH_PRONOUN_I = "english_pronoun_i"
    LETTER_I_CONTROL = "letter_i_control"
    I_CONTRACTION = "i_contraction"
    UTTERANCE_INITIAL_CAP = "utterance_initial_cap"


# ─── Stanza output projection ────────────────────────────────────────


@dataclass(frozen=True)
class StanzaWordResult:
    """Typed projection of a single Stanza UD word's morphotag output.

    One or more of these compose a :class:`StanzaTokenOutput`. Fields
    are the subset BA3 actually injects into %mor/%gra; richer
    annotations can be added as needs arise.
    """

    text: str
    upos: str
    lemma: str
    deprel: str


@dataclass(frozen=True)
class StanzaTokenOutput:
    """One pre-MWT input token and its post-MWT UD word expansion.

    For regular (non-MWT) tokens, ``words`` has length 1. For
    MWT-expanded tokens (``i'll`` → (``i``, ``will``), ``au`` →
    (``à``, ``le``)), ``words`` has length ≥ 2 with Stanza's
    expanded forms.

    Addressing probes at the token level (rather than at the UD-word
    level) keeps case authorship 1-to-1 with the pre-tokenized input
    word list regardless of MWT behavior.
    """

    text: str
    words: tuple[StanzaWordResult, ...]


# ─── Gold — per-side expected output ─────────────────────────────────


@dataclass(frozen=True)
class Gold:
    """Expected Stanza output for the pre-side and/or post-side of a
    :class:`TokenMapping`, at UD-word granularity.

    Each field, if set, is a tuple with one entry per UD word on that
    side. The comparator checks each set field independently: pre_upos
    describes expected UPOS for pre's flattened UD words; post_upos
    for post's. Lengths must match the number of UD words emitted on
    that side after MWT expansion — Stanza is the source of truth for
    that count.

    A side with all four fields None means "no expectation on this
    side" — the comparator treats that side as matching gold
    trivially. This is how 1-to-0 deletions are expressed: the pre
    side has an expectation, the post side has none (gold can't
    check an empty output).

    At least one of the four fields must be set on the whole ``Gold``
    object; an all-None Gold carries no signal and is rejected.
    """

    pre_upos: tuple[str, ...] | None = None
    post_upos: tuple[str, ...] | None = None
    pre_text: tuple[str, ...] | None = None
    post_text: tuple[str, ...] | None = None

    def __post_init__(self) -> None:
        if (
            self.pre_upos is None
            and self.post_upos is None
            and self.pre_text is None
            and self.post_text is None
        ):
            raise ValueError(
                "Gold requires at least one of pre_upos/post_upos/"
                "pre_text/post_text to be set. An all-None Gold "
                "carries no signal."
            )

    def has_pre_expectation(self) -> bool:
        return self.pre_upos is not None or self.pre_text is not None

    def has_post_expectation(self) -> bool:
        return self.post_upos is not None or self.post_text is not None


@dataclass(frozen=True)
class TokenMapping:
    """An n-to-m alignment between pre-tokens and post-tokens of a
    :class:`DecisionProbeCase`.

    * 1-to-1: standard case (``Dr.`` → ``Dr``).
    * 1-to-0: deletion (``saw him . /end`` → ``saw him``); the
      pre-side period disappears from the post form. Expressed by
      empty ``post_token_indices``.
    * 1-to-2: split (``Dr.`` → ``Dr`` + ``.``); one pre-token becomes
      two post-tokens.
    * 0-to-1: insertion (rare in normalization; included for
      symmetry).
    * 2-to-1 / n-to-m: merging.

    The mapping carries its own :class:`Gold`, so cases with multiple
    independent affected regions (e.g. both an I-cap and a period
    strip in the same utterance) can carry separate golds per
    region.

    At least one of the two index tuples must be non-empty — a
    0-to-0 mapping carries no signal.
    """

    pre_token_indices: tuple[int, ...]
    post_token_indices: tuple[int, ...]
    gold: Gold

    def __post_init__(self) -> None:
        if not self.pre_token_indices and not self.post_token_indices:
            raise ValueError(
                "TokenMapping requires at least one of "
                "pre_token_indices/post_token_indices to be non-empty."
            )


# ─── Case and comparison records ─────────────────────────────────────


@dataclass(frozen=True)
class DecisionProbeCase:
    """A single normalization-decision probe case (v2).

    Attributes
    ----------
    label
        Short identifier for pytest IDs and book tables.
    utterance_prose
        Plain-prose form of the underlying utterance for the
        contributor reading a failure.
    pre_words / post_words
        Pre-tokenized word sequences BEFORE and AFTER the proposed
        normalization. Stanza receives these as token lists.
    affected_mappings
        One or more :class:`TokenMapping` records describing which
        pre-tokens correspond to which post-tokens, and what gold
        applies per region. A case can have multiple mappings when
        the transformation affects multiple disjoint regions.
    expected_outcome
        Declared verdict; ``OBSERVE_ONLY`` during Phase 3 seeding
        before a case's outcome is locked.
    rationale
        Why this case exists and (once locked) what supports the
        verdict.
    candidate_class
        Which normalization rule the probe speaks to.
    xfail
        If set, the runner marks itself xfail with the defect slug.
    """

    label: str
    utterance_prose: str
    pre_words: tuple[str, ...]
    post_words: tuple[str, ...]
    affected_mappings: tuple[TokenMapping, ...]
    expected_outcome: DecisionOutcome
    rationale: str
    candidate_class: CandidateClass
    xfail: XfailMark | None = None

    def __post_init__(self) -> None:
        if not self.affected_mappings:
            raise ValueError(
                f"{self.label}: DecisionProbeCase requires at least one "
                "TokenMapping in affected_mappings"
            )


@dataclass(frozen=True)
class DecisionComparison:
    """Result of comparing Stanza outputs across all mappings of a case."""

    observed_outcome: DecisionOutcome
    notes: str


# ─── Per-mapping verdict (internal) ──────────────────────────────────


class _MappingVerdict(Enum):
    """Per-mapping classification. Aggregated to a
    :class:`DecisionOutcome` at the case level."""

    IMPROVED = "improved"
    REGRESSED = "regressed"
    NEUTRAL = "neutral"


# ─── Gold-side checking ──────────────────────────────────────────────


def _words_for(
    tokens: tuple[StanzaTokenOutput, ...],
    token_indices: tuple[int, ...],
    side: str,
) -> tuple[StanzaWordResult, ...]:
    """Flatten UD words for a list of token indices.

    Parameters
    ----------
    side
        "pre" or "post" — used only for the bounds-check error
        message so the offender is obvious.
    """
    out: list[StanzaWordResult] = []
    for i in token_indices:
        if i < 0 or i >= len(tokens):
            raise ValueError(
                f"{side}_token_indices contains out-of-range index {i}; "
                f"{side}_tokens has {len(tokens)} entries."
            )
        out.extend(tokens[i].words)
    return tuple(out)


def _side_matches_gold(
    words: tuple[StanzaWordResult, ...],
    expected_upos: tuple[str, ...] | None,
    expected_text: tuple[str, ...] | None,
) -> bool:
    """True if *every* set expectation matches the flattened UD words
    slot-for-slot. Absent expectations (None) are no-ops."""
    if expected_upos is not None:
        if len(expected_upos) != len(words):
            return False
        if any(w.upos != exp for w, exp in zip(words, expected_upos, strict=True)):
            return False
    if expected_text is not None:
        if len(expected_text) != len(words):
            return False
        if any(w.text != exp for w, exp in zip(words, expected_text, strict=True)):
            return False
    return True


def _classify_mapping(
    mapping: TokenMapping,
    pre_tokens: tuple[StanzaTokenOutput, ...],
    post_tokens: tuple[StanzaTokenOutput, ...],
) -> tuple[_MappingVerdict, str]:
    """Classify one mapping's pre/post match against its Gold.

    Returns the verdict and a short human-readable note.

    Rules:
    * If gold has no pre expectation, pre is treated as matching
      (the gold doesn't speak to the pre side).
    * Symmetric for post.
    * pre_matches and post_matches → NEUTRAL.
    * not pre_matches, post_matches → IMPROVED.
    * pre_matches, not post_matches → REGRESSED.
    * neither matches → NEUTRAL (both "wrong in some way" is not
      evidence that the transformation helps or hurts).
    """
    pre_words = _words_for(pre_tokens, mapping.pre_token_indices, "pre")
    post_words = _words_for(post_tokens, mapping.post_token_indices, "post")

    pre_matches = (
        not mapping.gold.has_pre_expectation()
        or _side_matches_gold(pre_words, mapping.gold.pre_upos, mapping.gold.pre_text)
    )
    post_matches = (
        not mapping.gold.has_post_expectation()
        or _side_matches_gold(post_words, mapping.gold.post_upos, mapping.gold.post_text)
    )

    pre_text = " ".join(w.text for w in pre_words) or "∅"
    post_text = " ".join(w.text for w in post_words) or "∅"
    pre_upos = "/".join(w.upos for w in pre_words) or "∅"
    post_upos = "/".join(w.upos for w in post_words) or "∅"

    if pre_matches and post_matches:
        verdict = _MappingVerdict.NEUTRAL
        tag = "both match"
    elif not pre_matches and post_matches:
        verdict = _MappingVerdict.IMPROVED
        tag = "post matches, pre does not"
    elif pre_matches and not post_matches:
        verdict = _MappingVerdict.REGRESSED
        tag = "pre matches, post does not"
    else:
        verdict = _MappingVerdict.NEUTRAL
        tag = "neither matches"
    note = (
        f"[{pre_text} ({pre_upos}) → {post_text} ({post_upos}): {tag}]"
    )
    return verdict, note


# ─── Top-level comparator ────────────────────────────────────────────


def compare_stanza_outputs(
    *,
    pre_tokens: tuple[StanzaTokenOutput, ...],
    post_tokens: tuple[StanzaTokenOutput, ...],
    mappings: tuple[TokenMapping, ...],
) -> DecisionComparison:
    """Classify the pre/post Stanza outputs for every mapping, then
    aggregate to a single case-level :class:`DecisionOutcome`.

    Aggregation:

    * all mapping verdicts NEUTRAL                → POST_NEUTRAL
    * any IMPROVED, no REGRESSED                  → POST_STRICTLY_BETTER
    * any REGRESSED, no IMPROVED                  → POST_STRICTLY_WORSE
    * at least one IMPROVED and one REGRESSED     → MIXED

    Raises
    ------
    ValueError
        If any mapping references an out-of-range token index. Catch
        at the case level so authorship mistakes surface at the test
        site rather than silently producing wrong verdicts.
    """
    verdicts: list[_MappingVerdict] = []
    notes_parts: list[str] = []
    for mapping in mappings:
        verdict, note = _classify_mapping(mapping, pre_tokens, post_tokens)
        verdicts.append(verdict)
        notes_parts.append(note)

    has_improved = any(v is _MappingVerdict.IMPROVED for v in verdicts)
    has_regressed = any(v is _MappingVerdict.REGRESSED for v in verdicts)
    if has_improved and has_regressed:
        outcome = DecisionOutcome.MIXED
    elif has_improved:
        outcome = DecisionOutcome.POST_STRICTLY_BETTER
    elif has_regressed:
        outcome = DecisionOutcome.POST_STRICTLY_WORSE
    else:
        outcome = DecisionOutcome.POST_NEUTRAL
    return DecisionComparison(observed_outcome=outcome, notes=" ".join(notes_parts))
