"""Phase 3 structural tests for the English decision-probe seed.

Phase 3 populates the decision matrix with observe-only English
cases covering every :class:`CandidateClass`. The tests here verify
the *shape* of the seed — coverage across candidate classes and
internal consistency of every case. They do not run Stanza and do
not assert any linguistic verdict; Stanza-driven verdicts enter in
Phase 3 step 3 (adjudication) once the golden matrix has run on a
machine with real models.

Why this is structural-only
---------------------------
The whole point of Stanza-driven probes is to discover outcomes
empirically rather than asserting them from the author's
expectation. An author-written outcome assertion before running
Stanza would simply encode author bias — exactly what
``feedback_empirical_before_assertions`` warns against. So Phase 3
ships the *cases* here; the *verdicts* (non-``OBSERVE_ONLY``
expected outcomes) are set in a follow-up commit after the golden
run on ming.
"""

from __future__ import annotations

from batchalign.tests.investigations._cases import ENG
from batchalign.tests.investigations._decision_cases import (
    DECISION_LANGUAGE_MATRIX,
    all_decision_cases,
)
from batchalign.tests.investigations._decision_probe_types import (
    CandidateClass,
)


def test_english_is_registered() -> None:
    """Phase 3 seeds English; other languages join later."""
    assert ENG in DECISION_LANGUAGE_MATRIX
    assert len(DECISION_LANGUAGE_MATRIX[ENG]) > 0


def test_every_candidate_class_has_at_least_one_english_case() -> None:
    """The seed's coverage contract: every rule the harness knows
    about must be probed. If a new ``CandidateClass`` enters without
    an English case, this test forces the seed author to add one."""
    english_cases = DECISION_LANGUAGE_MATRIX[ENG]
    classes_seen = {c.candidate_class for c in english_cases}
    missing = set(CandidateClass) - classes_seen
    assert not missing, (
        f"English seed is missing CandidateClass coverage: "
        f"{sorted(c.value for c in missing)}"
    )


def test_every_case_has_token_indices_in_range() -> None:
    """Catches mapping-index typos at collection time rather than
    inside the comparator at runtime."""
    for lang, case in all_decision_cases():
        for m_idx, mapping in enumerate(case.affected_mappings):
            for i in mapping.pre_token_indices:
                assert 0 <= i < len(case.pre_words), (
                    f"{lang.alpha3} {case.label} mapping#{m_idx}: "
                    f"pre_token_indices has out-of-range index {i} for "
                    f"pre_words of length {len(case.pre_words)}"
                )
            for i in mapping.post_token_indices:
                assert 0 <= i < len(case.post_words), (
                    f"{lang.alpha3} {case.label} mapping#{m_idx}: "
                    f"post_token_indices has out-of-range index {i} for "
                    f"post_words of length {len(case.post_words)}"
                )


def test_every_case_has_at_least_one_mapping() -> None:
    """A case with no mappings exercises nothing and is meaningless."""
    for lang, case in all_decision_cases():
        assert case.affected_mappings, (
            f"{lang.alpha3} {case.label}: affected_mappings must be non-empty"
        )


def test_every_case_has_rationale() -> None:
    """Rationale is the succession-time evidence for the case's
    existence. Empty rationale defeats the whole point of the
    decision-probe model."""
    for lang, case in all_decision_cases():
        assert case.rationale.strip(), (
            f"{lang.alpha3} {case.label}: rationale must not be empty"
        )
