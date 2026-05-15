"""Shared Stanza pipeline fixtures for the CHAT-input investigation harness.

The investigation probes parametrize tests over ``(LanguageKey,
ProbeCase)`` pairs. Each test needs a Stanza pipeline matching
the pair's language — loaded once per language per session, not
per test invocation (pipeline load is ~5s).

The two fixtures here (:func:`post_pipeline_for` and
:func:`free_pipeline_for`) are module-scoped factories: they
return a callable that takes a ``LanguageKey`` and returns the
appropriate pipeline, caching per session in a closure dict.
Adding a new language requires no changes here — the factory
builds the pipeline on first request using the language's
alpha-2 code and the shared capability-aware processor selector
:func:`_processors_for`.
"""

from __future__ import annotations

import pytest

from batchalign.inference._tokenizer_realign import (
    TokenizerContext,
    make_tokenizer_postprocessor,
)

# Stanza is imported lazily inside the pipeline-builder helpers below.
# Importing stanza at module scope costs ~1-2s on every pytest
# collection (torch/numpy/transformers cascade), even for runs that
# never touch these fixtures. The builders only run when a fixture is
# resolved, so deferring the import saves collection time unconditionally.


def _processors_for(lang: str) -> str:
    """Return the Stanza processor list appropriate for ``lang``.

    Not every language ships an MWT model (e.g. Russian). We ask the
    production capability registry
    (``batchalign.worker._stanza_capabilities``) which already reads
    and caches Stanza's ``resources.json`` at worker startup, so the
    fixture and production paths share one source of truth for
    "does Stanza support MWT for this language?"

    If the capability registry is unavailable (Stanza not installed
    at test collection time), fall back to requesting ``mwt``
    unconditionally — Stanza will error loudly on the first fixture
    setup if it can't satisfy the request, which is informative
    rather than silent.
    """
    base = ["tokenize", "pos", "lemma", "depparse"]
    from batchalign.worker._stanza_capabilities import get_cached_capability_table

    table = get_cached_capability_table()
    if table is None:
        base.append("mwt")
        return ",".join(base)
    cap = table.languages.get(lang)
    if cap is not None and cap.has_mwt:
        base.append("mwt")
    return ",".join(base)


def _pipeline_with_postprocessor(alpha2: str) -> tuple:
    """Build a (nlp, ctx) pair with BA3's realignment postprocessor.

    Simulates production Preserve-mode: the ``TokenizerContext`` is
    populated with the CHAT word list before each ``nlp()`` call so
    the BA2-ported MWT override table applies.
    """
    import stanza
    from stanza import DownloadMethod

    ctx = TokenizerContext()
    pp = make_tokenizer_postprocessor(ctx, alpha2=alpha2)
    nlp = stanza.Pipeline(
        lang=alpha2,
        processors=_processors_for(alpha2),
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        tokenize_postprocessor=pp,
        verbose=False,
    )
    return nlp, ctx


def _pipeline_free(alpha2: str):
    """Stanza pipeline WITHOUT our realignment postprocessor.

    Use this to observe what Stanza emits absent BA3's override
    layer. Baseline for deciding whether a mismatch is Stanza-native
    or hack-introduced.
    """
    import stanza
    from stanza import DownloadMethod

    return stanza.Pipeline(
        lang=alpha2,
        processors=_processors_for(alpha2),
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        verbose=False,
    )


# ─── Matrix harness resolver fixtures ───────────────────────────────
#
# The matrix-driven probe harness parametrizes tests over
# ``(LanguageKey, ProbeCase)`` pairs. Each test calls the resolver
# below with its ``LanguageKey`` to get the pipeline; the resolver
# caches per session so each Stanza pipeline loads exactly once.


@pytest.fixture(scope="module")
def post_pipeline_for():
    """Factory: ``LanguageKey → (nlp, ctx)`` with postprocessor active.

    Pipelines load lazily on first request per language and are
    cached in the factory's closure for the duration of the test
    session.
    """
    cache: dict[str, tuple] = {}

    def resolver(lang_key) -> tuple:
        key = lang_key.alpha2
        if key not in cache:
            cache[key] = _pipeline_with_postprocessor(key)
        return cache[key]

    return resolver


@pytest.fixture(scope="module")
def free_pipeline_for():
    """Factory: ``LanguageKey → nlp`` with no postprocessor."""
    cache: dict[str, object] = {}

    def resolver(lang_key):
        key = lang_key.alpha2
        if key not in cache:
            cache[key] = _pipeline_free(key)
        return cache[key]

    return resolver
