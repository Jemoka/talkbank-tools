"""Matrix-driven Stanza decision-probe runner (v2, token-centric).

For every :class:`DecisionProbeCase` in the decision matrix this
runs the language's with-postprocessor Stanza pipeline on both
``pre_words`` and ``post_words``, builds token-centric
:class:`StanzaTokenOutput` tuples from ``doc.sentences[*].tokens``,
calls :func:`compare_stanza_outputs`, prints observed vs. expected,
and asserts (or xfails, or short-circuits on OBSERVE_ONLY).

Token-centric addressing
------------------------
Earlier v1 flattened ``doc.sentences[*].words`` and indexed by the
case author's pre-tokenized input position. That silently broke on
MWT expansions (``i'll`` → ``i`` + ``will`` shifts every later
index). v2 addresses **pre-MWT tokens** via
``doc.sentences[*].tokens`` (which are 1-to-1 with the input word
list when ``tokenize_no_ssplit`` is True and the tokenizer does not
split further), and carries each token's post-MWT UD word tuple as
``token.words``.

Marked ``@pytest.mark.golden`` so it runs only on machines with
real Stanza models.
"""

from __future__ import annotations

from typing import Any, Callable

import pytest

from ._cases import LanguageKey
from ._decision_cases import all_decision_cases
from ._decision_probe_types import (
    DecisionOutcome,
    DecisionProbeCase,
    StanzaTokenOutput,
    StanzaWordResult,
    compare_stanza_outputs,
)


def _stanza_doc_to_tokens(doc: Any) -> tuple[StanzaTokenOutput, ...]:
    """Flatten a Stanza Document into our token-centric projection.

    Walks ``doc.sentences[*].tokens`` — these are pre-MWT tokens,
    1-to-1 with the input word list under ``tokenize_no_ssplit`` +
    our realignment postprocessor. For each token, collects its
    ``.words`` (post-MWT UD words) into a :class:`StanzaTokenOutput`.

    Note: the returned sequence may not be 1-to-1 with the input
    word list if Stanza's tokenizer further splits an input word
    (rare with our postprocessor active). The caller's mapping
    indices are trusted to reflect Stanza's actual token count;
    out-of-range indices raise at comparator time with a clear
    message.
    """
    out: list[StanzaTokenOutput] = []
    for sent in doc.sentences:
        for token in sent.tokens:
            words = tuple(
                StanzaWordResult(
                    text=w.text,
                    upos=w.upos or "",
                    lemma=w.lemma or "",
                    deprel=getattr(w, "deprel", "") or "",
                )
                for w in token.words
            )
            out.append(StanzaTokenOutput(text=token.text, words=words))
    return tuple(out)


def _case_id(pair: tuple[LanguageKey, DecisionProbeCase]) -> str:
    lang, case = pair
    return f"{lang.alpha3}__{case.candidate_class.value}__{case.label}"


_MATRIX = all_decision_cases()


@pytest.mark.golden
@pytest.mark.decision_probe
@pytest.mark.parametrize("pair", _MATRIX, ids=_case_id)
def test_stanza_decision_probe(
    pair: tuple[LanguageKey, DecisionProbeCase],
    post_pipeline_for: Callable[[LanguageKey], tuple[Any, Any]],
) -> None:
    """Run a decision-probe case through the with-postprocessor
    pipeline, build token-centric Stanza outputs, and assert the
    comparator's observed outcome matches the declared expectation.
    """
    lang, case = pair
    nlp, ctx = post_pipeline_for(lang)

    ctx.original_words = [list(case.pre_words)]
    try:
        pre_doc = nlp(" ".join(case.pre_words))
    finally:
        ctx.original_words = []
    ctx.original_words = [list(case.post_words)]
    try:
        post_doc = nlp(" ".join(case.post_words))
    finally:
        ctx.original_words = []

    pre_tokens = _stanza_doc_to_tokens(pre_doc)
    post_tokens = _stanza_doc_to_tokens(post_doc)

    comparison = compare_stanza_outputs(
        pre_tokens=pre_tokens,
        post_tokens=post_tokens,
        mappings=case.affected_mappings,
    )

    print(
        f"\n{lang.alpha3} {case.candidate_class.value} {case.label}: "
        f"observed={comparison.observed_outcome.value} "
        f"expected={case.expected_outcome.value} "
        f"notes={comparison.notes}"
    )

    if case.xfail is not None:
        pytest.xfail(f"{case.xfail.defect_slug}: {case.xfail.reason}")
    if case.expected_outcome is DecisionOutcome.OBSERVE_ONLY:
        return
    assert comparison.observed_outcome is case.expected_outcome, (
        f"{lang.alpha3} {case.label}: "
        f"expected {case.expected_outcome.value}, "
        f"got {comparison.observed_outcome.value}; notes={comparison.notes}"
    )
