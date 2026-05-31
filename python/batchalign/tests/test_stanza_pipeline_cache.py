"""Tests for the per-(langset, mode) Stanza pipeline cache.

We don't import stanza here — it's heavy and we just want to exercise
the cache shape. Patch `stanza` and the `_build_pipeline` machinery and
assert the cache is hit on the second backend.
"""

from __future__ import annotations

from unittest import mock

import pytest


def _reset_cache() -> None:
    from batchalign.backends.morphosyntax import stanza as stanza_backend_mod

    stanza_backend_mod._pipeline_cache.clear()


def test_pipeline_cache_hits_on_same_key() -> None:
    """Second backend with same langs+retokenize reuses the first pipeline."""
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.return_value = mock.sentinel.PIPE_EN
    fake_stanza.__version__ = "test"

    with mock.patch.dict("sys.modules", {"stanza": fake_stanza}):
        a = StanzaBackend(lang="en", retokenize=False)
        b = StanzaBackend(lang="en", retokenize=False)

    assert a._nlp is mock.sentinel.PIPE_EN
    assert b._nlp is mock.sentinel.PIPE_EN
    # Pipeline was constructed exactly once across both backends.
    assert fake_stanza.Pipeline.call_count == 1


def test_pipeline_cache_misses_on_different_retokenize() -> None:
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.side_effect = [
        mock.sentinel.PIPE_NORETOK,
        mock.sentinel.PIPE_RETOK,
    ]
    fake_stanza.__version__ = "test"

    with mock.patch.dict("sys.modules", {"stanza": fake_stanza}):
        a = StanzaBackend(lang="en", retokenize=False)
        b = StanzaBackend(lang="en", retokenize=True)

    assert a._nlp is mock.sentinel.PIPE_NORETOK
    assert b._nlp is mock.sentinel.PIPE_RETOK
    assert fake_stanza.Pipeline.call_count == 2


def test_pipeline_cache_misses_on_different_langs() -> None:
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.side_effect = [
        mock.sentinel.PIPE_EN,
        mock.sentinel.PIPE_ES,
    ]
    fake_stanza.__version__ = "test"

    with mock.patch.dict("sys.modules", {"stanza": fake_stanza}):
        a = StanzaBackend(lang="en", retokenize=False)
        b = StanzaBackend(lang="es", retokenize=False)

    assert a._nlp is mock.sentinel.PIPE_EN
    assert b._nlp is mock.sentinel.PIPE_ES
    assert fake_stanza.Pipeline.call_count == 2


def test_multilingual_pipeline_cached_under_frozenset_key() -> None:
    """Two backends with same lang set (any order) share the multilingual pipeline."""
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.MultilingualPipeline.return_value = mock.sentinel.PIPE_MULTI
    fake_stanza.__version__ = "test"

    with mock.patch.dict("sys.modules", {"stanza": fake_stanza}):
        a = StanzaBackend(lang="en,es", retokenize=False)
        b = StanzaBackend(lang="es,en", retokenize=False)

    assert a._nlp is mock.sentinel.PIPE_MULTI
    assert b._nlp is mock.sentinel.PIPE_MULTI
    assert fake_stanza.MultilingualPipeline.call_count == 1
