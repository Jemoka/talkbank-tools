"""Matrix-driven Stanza MWT probe harness.

One test module for all languages. Reads the typed
:data:`_cases.LANGUAGE_MATRIX` registry and parametrizes every
:class:`_probe_types.ProbeCase` through the paired pipelines:

* ``test_stanza_mwt_probe_free_tokenize`` — no postprocessor active.
  Always observe-only: pins what Stanza's native tokenizer + MWT
  processor emit for the case. A Stanza-version upgrade that changes
  behavior surfaces as a diff.

* ``test_stanza_mwt_probe_with_postprocessor`` — BA3's tokenizer
  postprocessor (char-DP realigner, no per-language override rules
  as of 2026-04-22) active. Enforces the case's
  ``expected_post_mwt_count`` if set, observes otherwise. Xfail cases
  marked via :class:`_probe_types.XfailMark` surface the pinned
  Stanza limitation without failing the suite.

Both test functions are marked ``@pytest.mark.golden`` so they don't
run in the default test suite — they require real Stanza models.

History
-------
Before 2026-04-22 these probes lived in 5 hand-written modules under
``batchalign/tests/investigations/test_stanza_*.py``. The typed-matrix
consolidation replaced them with this single module + the data-only
``_cases/<lang>.py`` fixtures. See
``book/src/reference/languages/<lang>.md`` for the per-language audit
records the probes support.
"""

from __future__ import annotations

from typing import Any, Callable

import pytest

from ._cases import LanguageKey, all_cases
from ._probe_types import ProbeCase


def _token_summary(doc: Any) -> list[tuple[str, str, str]]:
    """Flatten a Stanza Document to ``(text, upos, lemma)`` triples.

    The lemma column exists so lemma-quality defects (e.g. Italian
    Defect 8 surface-echo: ``mettilo`` → lemma ``mettilo`` instead of
    ``mettere``) are visible alongside the POS-layer defects the
    original ``(text, upos)`` pairs surface. Word.lemma can legally be
    ``None`` for unknown tokens; fall back to the literal string
    ``"None"`` so the probe-log line stays a tuple of strings.
    """
    return [
        (word.text, word.upos, word.lemma if word.lemma is not None else "None")
        for sent in doc.sentences
        for word in sent.words
    ]


def _post_mwt_count(doc: Any) -> int:
    """Number of UD ``words`` across all sentences after MWT expansion."""
    return sum(len(s.words) for s in doc.sentences)


def _case_id(pair: tuple[LanguageKey, ProbeCase]) -> str:
    """Pytest test-ID formatter: ``<alpha3>__<label>``."""
    lang, case = pair
    return f"{lang.alpha3}__{case.label}"


_MATRIX = all_cases()


@pytest.mark.golden
@pytest.mark.mwt_probe
@pytest.mark.parametrize("pair", _MATRIX, ids=_case_id)
def test_stanza_mwt_probe_free_tokenize(
    pair: tuple[LanguageKey, ProbeCase],
    free_pipeline_for: Callable[[LanguageKey], Any],
) -> None:
    """Observe-only pin of Stanza's free-tokenize output for every case.

    A Stanza upgrade that changes tokenization surfaces here as
    print-only output (no assertion). Pair with the
    with-postprocessor variant to diff the two paths.
    """
    lang, case = pair
    nlp = free_pipeline_for(lang)
    text = " ".join(case.words)
    doc = nlp(text)
    observed = _token_summary(doc)
    print(
        f"\n{case.label} {lang.alpha2} free "
        f"(chat={len(case.words)}) stanza_words={_post_mwt_count(doc)} "
        f"observed={observed}"
    )


@pytest.mark.golden
@pytest.mark.mwt_probe
@pytest.mark.parametrize("pair", _MATRIX, ids=_case_id)
def test_stanza_mwt_probe_with_postprocessor(
    pair: tuple[LanguageKey, ProbeCase],
    post_pipeline_for: Callable[[LanguageKey], tuple[Any, Any]],
) -> None:
    """Invariant-enforcing probe with our tokenizer postprocessor active.

    Enforces ``case.expected_post_mwt_count`` if set; observes
    otherwise. ``case.xfail`` cases mark themselves xfail with the
    defect slug so the failure is linked to its registered Stanza
    limitation.
    """
    lang, case = pair
    nlp, ctx = post_pipeline_for(lang)
    ctx.original_words = [list(case.words)]
    try:
        doc = nlp(" ".join(case.words))
    finally:
        ctx.original_words = []
    observed = _token_summary(doc)
    stanza_count = _post_mwt_count(doc)
    print(
        f"\n{case.label} {lang.alpha2} post "
        f"(chat={len(case.words)}) stanza_words={stanza_count} "
        f"observed={observed}"
    )
    if case.xfail is not None:
        pytest.xfail(
            f"{case.xfail.defect_slug}: {case.xfail.reason} — "
            f"observed={observed}"
        )
    if case.expected_post_mwt_count is not None:
        assert stanza_count == case.expected_post_mwt_count, (
            f"{lang.alpha3} {case.label}: "
            f"CHAT={len(case.words)} "
            f"expected_stanza={case.expected_post_mwt_count} "
            f"got_stanza={stanza_count} observed={observed}"
        )
