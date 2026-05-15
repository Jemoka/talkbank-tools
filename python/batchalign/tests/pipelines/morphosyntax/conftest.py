"""Shared Stanza pipeline fixtures for morphosyntax tests.

All three pipeline variants mirror ``_stanza_loading.py`` so tests exercise
the same configuration as production workers (processor order, MWT
package, tokenize flags).
"""

from __future__ import annotations

import pytest

from batchalign.inference._tokenizer_realign import (
    TokenizerContext,
    make_tokenizer_postprocessor,
)

# Stanza is imported lazily inside each fixture. Importing at module
# scope costs ~1-2s on every pytest collection (torch/numpy/transformers
# cascade), even for tests that don't touch Stanza at all. Fixtures run
# only when their tests are selected, so lazy import is free elsewhere.

# Processor list that matches `_stanza_loading.py:93` + ",mwt" append for
# MWT-capable languages (English among them). Stanza runs processors in
# their fixed internal dependency order regardless of list order; we use
# the production order here for transparency.
_ENGLISH_PROCESSORS = "tokenize,pos,lemma,depparse,mwt"


@pytest.fixture(scope="module")
def english_pipeline_with_postprocessor():
    """BA3 English pipeline — mirrors ``_stanza_loading.py:124-132``.

    The postprocessor consults ``ctx.original_words`` to decide whether
    to reverse Stanza's MWT splits.
    """
    import stanza
    from stanza import DownloadMethod

    ctx = TokenizerContext()
    pp = make_tokenizer_postprocessor(ctx, alpha2="en")
    nlp = stanza.Pipeline(
        lang="en",
        processors=_ENGLISH_PROCESSORS,
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        tokenize_postprocessor=pp,
        package={"mwt": "gum"},
        verbose=False,
    )
    return nlp, ctx


@pytest.fixture(scope="module")
def english_pipeline_free_tokenize():
    """English pipeline with NO postprocessor — Stanza unconstrained.

    Baseline for what Stanza-with-MWT produces absent our realignment
    layer.
    """
    import stanza
    from stanza import DownloadMethod

    nlp = stanza.Pipeline(
        lang="en",
        processors=_ENGLISH_PROCESSORS,
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        package={"mwt": "gum"},
        verbose=False,
    )
    return nlp


@pytest.fixture(scope="module")
def english_pipeline_pretokenized():
    """English pipeline with ``tokenize_pretokenized=True``.

    Used to verify that forcing pretokenized input suppresses MWT
    expansion — Stanza's MWT processor skips re-splitting tokens that
    were declared pre-tokenized.
    """
    import stanza
    from stanza import DownloadMethod

    nlp = stanza.Pipeline(
        lang="en",
        processors=_ENGLISH_PROCESSORS,
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        tokenize_pretokenized=True,
        package={"mwt": "gum"},
        verbose=False,
    )
    return nlp
