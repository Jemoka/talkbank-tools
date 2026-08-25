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
    stanza_backend_mod._pipeline_failures.clear()


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


def test_hindi_pipeline_does_not_request_missing_mwt_model() -> None:
    """Hindi's Stanza package has no MWT processor/model."""
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.return_value = mock.sentinel.PIPE_HI
    fake_stanza.__version__ = "test"

    with mock.patch.dict("sys.modules", {"stanza": fake_stanza}):
        backend = StanzaBackend(lang="hin", retokenize=False)

    assert backend._nlp is mock.sentinel.PIPE_HI
    processors = fake_stanza.Pipeline.call_args.kwargs["processors"]
    assert processors == {
        "tokenize": "default",
        "pos": "default",
        "lemma": "default",
        "depparse": "default",
    }


@pytest.mark.parametrize("surface", ["जी", "हाँ", "தமிழ்"])
def test_code_switch_masker_keeps_unicode_combining_marks(surface: str) -> None:
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    line, special_forms = StanzaBackend._preprocess_text(
        f"before {surface}@s after"
    )

    assert line == "before xbxxx after"
    assert special_forms == [[surface, "s"]]


def test_one_bad_language_does_not_kill_the_batch() -> None:
    """A pipeline-init failure for one input must NOT fail the rest of the batch.

    This is the failure mode that surfaced as "Language su unsupported"
    aborting every file in a multi-file run: the batcher mixes utterances
    from different files into one `backend.call(batch)`, so any exception
    raised by `call` is broadcast to every reply in the batch (see
    `crates/batchalign/batchalign-engine/src/batcher.rs:150`).
    """
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend
    from batchalign._core.proto import (
        LanguageSpecCode,
        MorphosyntaxInput,
    )

    fake_stanza = mock.MagicMock()

    fake_stanza.__version__ = "test"

    # The `eng` pipeline is wired to return an empty `doc.sentences` for
    # whatever joined input it receives; the renderer then yields an empty
    # SentenceAnalysis. We only care that the dispatcher (a) doesn't crash
    # when one language is broken, and (b) attempts the broken pipeline at
    # most once.
    def _make_pipeline(_lang: str, **_: object):
        pipe = mock.MagicMock(name=f"PIPE_{_lang}")
        pipe.return_value = mock.MagicMock(sentences=[])
        return pipe

    fake_stanza.Pipeline.side_effect = lambda lang, **kw: (
        _make_pipeline(lang) if lang != "su" else (_ for _ in ()).throw(
            ValueError("No processors to load for language su.")
        )
    )

    with mock.patch.dict("sys.modules", {"stanza": fake_stanza}):
        backend = StanzaBackend()  # unpinned — language flows from inputs

        inputs = [
            MorphosyntaxInput(
                source_id="ok.cha",
                utterance_id=0,
                language=LanguageSpecCode(kind="code", value="eng"),
                tokens=["hi"],
                retokenize=False,
                text="hi",
            ),
            MorphosyntaxInput(
                source_id="bad.cha",
                utterance_id=0,
                language=LanguageSpecCode(kind="code", value="sun"),
                tokens=["xxx"],
                retokenize=False,
                text="xxx",
            ),
            MorphosyntaxInput(
                source_id="bad.cha",
                utterance_id=1,
                language=LanguageSpecCode(kind="code", value="sun"),
                tokens=["yyy"],
                retokenize=False,
                text="yyy",
            ),
        ]

        outputs = backend.call(inputs)

    # Every input gets an output. The failing-language inputs come back
    # with empty tokens (runner treats those as "no %mor for this utt").
    assert len(outputs) == len(inputs)
    assert outputs[0].source_id == "ok.cha"
    assert outputs[1].tokens == []
    assert outputs[2].tokens == []
    # The broken language was attempted exactly once across all three
    # inputs — second/third utterances short-circuit via the memo.
    su_attempts = [
        c for c in fake_stanza.Pipeline.call_args_list if c.kwargs.get("lang") == "su"
    ]
    assert len(su_attempts) == 1, (
        f"expected exactly one Stanza.Pipeline(lang='su') call, got {len(su_attempts)}"
    )


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
