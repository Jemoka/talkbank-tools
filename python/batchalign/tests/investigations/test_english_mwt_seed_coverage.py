"""Structural coverage tests for the English MWT probe seed.

BA2-jan9 had an English apostrophe-contraction rule in
``tokenizer_processor()`` (ud.py:694-697): any token containing an
apostrophe was marked for contraction-preserving treatment except
``o'`` + word (so ``o'clock`` was excluded). BA3 does not have a
direct equivalent; Stanza's native English MWT processor is
expected to handle these.

The 2026-04-23 retokenization-gap audit
(``docs/investigations/2026-04-23-retokenization-gap-plan.md``)
surfaced that English had **zero MWT probe coverage** — the
existing ``_cases/english.py`` is for decision probes (capitalization
rules), not tokenization. A silent regression to `don't` or
`o'clock` handling would not be caught by any default or golden
test.

These tests assert the English seed is present in the MWT matrix
and covers the phenomena BA2 explicitly gated:

* standard contractions (expect MWT expansion to 2 UD words)
* possessives (expect MWT expansion)
* ``o'clock`` (control; expect NO expansion — BA2 explicitly
  excluded this from its contraction rule)
* native MWT baselines (if Stanza has any for English)

Runner tests in ``test_stanza_mwt_probe_matrix.py`` assert the
``expected_post_mwt_count`` values per case during golden runs.
"""

from __future__ import annotations

from batchalign.tests.investigations._cases import (
    ENG,
    LANGUAGE_MATRIX,
    all_cases,
)
from batchalign.tests.investigations._probe_types import Phenomenon


def test_english_is_in_mwt_matrix() -> None:
    """BA2 had English apostrophe-clitic rules; BA3 must probe them."""
    assert ENG in LANGUAGE_MATRIX
    assert len(LANGUAGE_MATRIX[ENG]) > 0


def test_english_covers_contractions() -> None:
    """Standard English contractions (``don't``, ``I'm``, etc.) — what
    BA2's apostrophe rule was designed for."""
    eng_cases = LANGUAGE_MATRIX[ENG]
    contraction_cases = [
        c for c in eng_cases if c.phenomenon is Phenomenon.CONTRACTION
    ]
    assert len(contraction_cases) >= 3, (
        f"English MWT probes need at least 3 CONTRACTION cases; "
        f"found {len(contraction_cases)}"
    )


def test_english_covers_oclock_control() -> None:
    """``o'clock`` was BA2's explicit exclusion from the
    apostrophe-contraction rule. A control case must exist so any
    future change that accidentally MWT-expands ``o'clock`` is
    caught."""
    eng_cases = LANGUAGE_MATRIX[ENG]
    oclock_cases = [
        c for c in eng_cases if "oclock" in c.label or "o_clock" in c.label
    ]
    assert oclock_cases, (
        "English MWT seed must include an o'clock control case "
        "(BA2 explicitly excluded this from contraction handling)"
    )


def test_every_english_case_has_expected_count_or_xfail() -> None:
    """Tokenization probes with no assertion are weak probes. Every
    English case should either assert ``expected_post_mwt_count`` or
    carry an explicit xfail mark. Observe-only is acceptable only
    for NATIVE_MWT controls (where Stanza is the authority and we're
    just pinning behavior)."""
    for lang, case in all_cases():
        if lang is not ENG:
            continue
        if case.phenomenon is Phenomenon.NATIVE_MWT:
            continue
        has_assertion = case.expected_post_mwt_count is not None
        has_xfail = case.xfail is not None
        assert has_assertion or has_xfail, (
            f"English case {case.label} has neither expected count "
            f"nor xfail mark — it's observe-only and would miss "
            f"regressions"
        )
