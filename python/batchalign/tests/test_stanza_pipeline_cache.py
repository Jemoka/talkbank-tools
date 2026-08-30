"""Tests for the per-(langset, mode) Stanza pipeline cache.

We don't import stanza here — it's heavy and we just want to exercise
the cache shape. Patch `stanza` and the `_build_pipeline` machinery and
assert the cache is hit on the second backend.
"""

from __future__ import annotations

from contextlib import contextmanager
from types import SimpleNamespace
from unittest import mock

import pytest


def _reset_cache() -> None:
    from batchalign.backends.morphosyntax import stanza as stanza_backend_mod

    stanza_backend_mod._pipeline_cache.clear()
    stanza_backend_mod._pipeline_failures.clear()


@contextmanager
def _fake_runtime(fake_stanza):
    """Patch the runtime loader without unloading Torch's C extension.

    ``patch.dict(sys.modules)`` removes modules imported inside its context on
    exit. Re-importing PyTorch's extension in the same process is unsupported
    and can segfault, so backend unit tests replace the narrow loader instead.
    """
    from batchalign.backends.morphosyntax import stanza as stanza_backend_mod

    with mock.patch.object(
        stanza_backend_mod,
        "_import_stanza_runtime",
        return_value=fake_stanza,
    ):
        yield


def test_stanza_runtime_eagerly_prepares_worker_thread_dependencies() -> None:
    """Prepare Torch and Stanza's download lock before engine threads start."""
    from batchalign.backends.morphosyntax import stanza as stanza_backend_mod

    imported: list[str] = []
    fake_stanza = mock.MagicMock()

    def _import(name: str):
        imported.append(name)
        return fake_stanza if name == "stanza" else mock.sentinel.TORCH_MODULE

    with mock.patch.object(stanza_backend_mod.importlib, "import_module", _import):
        result = stanza_backend_mod._import_stanza_runtime()

    assert result is fake_stanza
    assert imported == ["torch", "torch._functorch.config", "stanza"]
    fake_stanza.resources.common.tqdm.get_lock.assert_called_once_with()


def test_pipeline_cache_hits_on_same_key() -> None:
    """Second backend with same langs+retokenize reuses the first pipeline."""
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.return_value = mock.sentinel.PIPE_EN
    fake_stanza.__version__ = "test"

    with _fake_runtime(fake_stanza):
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

    with _fake_runtime(fake_stanza):
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

    with _fake_runtime(fake_stanza):
        a = StanzaBackend(lang="en", retokenize=False)
        b = StanzaBackend(lang="es", retokenize=False)

    assert a._nlp is mock.sentinel.PIPE_EN
    assert b._nlp is mock.sentinel.PIPE_ES
    assert fake_stanza.Pipeline.call_count == 2


def test_pipeline_cache_evicts_least_recently_used_language_set() -> None:
    """A corpus cannot retain every full Stanza model it encounters."""
    _reset_cache()
    from batchalign.backends.morphosyntax import stanza as stanza_backend_mod
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.side_effect = [
        mock.sentinel.PIPE_EN,
        mock.sentinel.PIPE_ES,
        mock.sentinel.PIPE_FR,
    ]
    fake_stanza.__version__ = "test"

    with (
        _fake_runtime(fake_stanza),
        mock.patch.object(stanza_backend_mod.gc, "collect") as collect,
    ):
        StanzaBackend(lang="en")
        StanzaBackend(lang="es")
        StanzaBackend(lang="en")
        StanzaBackend(lang="fr")

    assert len(stanza_backend_mod._pipeline_cache) == 2
    assert fake_stanza.Pipeline.call_count == 3
    collect.assert_called_once_with()
    assert (frozenset({"es"}), False) not in stanza_backend_mod._pipeline_cache
    assert (frozenset({"en"}), False) in stanza_backend_mod._pipeline_cache
    assert (frozenset({"fr"}), False) in stanza_backend_mod._pipeline_cache


def test_language_group_releases_postprocessor_sentences() -> None:
    """Completed inference must not retain the batch's source text."""
    _reset_cache()
    from batchalign.backends.morphosyntax import stanza as stanza_backend_mod
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.__version__ = "test"
    item = SimpleNamespace(
        text="ciao",
        tokens=["ciao"],
        source_id="sample.cha",
        utterance_id=0,
    )
    nlp = mock.MagicMock(
        return_value=SimpleNamespace(sentences=[mock.sentinel.SENTENCE])
    )
    outputs = [None]
    key = (frozenset({"it"}), False)

    with _fake_runtime(fake_stanza):
        backend = StanzaBackend()
    with (
        mock.patch.object(
            stanza_backend_mod.render,
            "parse_sentence",
            return_value=mock.sentinel.ANALYSIS,
        ),
        mock.patch.object(
            backend,
            "_analysis_to_output",
            return_value=mock.sentinel.OUTPUT,
        ),
    ):
        backend._run_language_group(
            key,
            ["it"],
            nlp,
            [(0, item, ("it",))],
            outputs,
        )

    assert outputs == [mock.sentinel.OUTPUT]
    assert stanza_backend_mod._current_sentences_for(key) == []


def test_hindi_pipeline_does_not_request_missing_mwt_model() -> None:
    """Hindi's Stanza package has no MWT processor/model."""
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    fake_stanza = mock.MagicMock()
    fake_stanza.Pipeline.return_value = mock.sentinel.PIPE_HI
    fake_stanza.__version__ = "test"

    with _fake_runtime(fake_stanza):
        backend = StanzaBackend(lang="hin", retokenize=False)

    assert backend._nlp is mock.sentinel.PIPE_HI
    processors = fake_stanza.Pipeline.call_args.kwargs["processors"]
    assert processors == {
        "tokenize": "default",
        "pos": "default",
        "lemma": "default",
        "depparse": "default",
    }
    assert fake_stanza.Pipeline.call_args.kwargs["download_method"] == "reuse_resources"


@pytest.mark.parametrize("surface", ["जी", "हाँ", "தமிழ்"])
def test_code_switch_masker_keeps_unicode_combining_marks(surface: str) -> None:
    from batchalign.backends.morphosyntax.stanza import StanzaBackend

    line, special_forms = StanzaBackend._preprocess_text(
        f"before {surface}@s after"
    )

    assert line == "before xbxxx after"
    assert special_forms == [[surface, "s"]]


def test_pipeline_init_failure_is_fatal_and_memoized() -> None:
    """A missing runtime/model must never look like successful empty tiers."""
    _reset_cache()
    from batchalign.backends.morphosyntax.stanza import StanzaBackend
    from batchalign._core.proto import (
        LanguageSpecCode,
        MorphosyntaxInput,
    )

    fake_stanza = mock.MagicMock()

    fake_stanza.__version__ = "test"

    # The `eng` pipeline is wired to return an empty `doc.sentences` for
    # whatever joined input it receives. We only care that the dispatcher
    # reports the broken language and attempts that pipeline at most once.
    def _make_pipeline(_lang: str, **_: object):
        pipe = mock.MagicMock(name=f"PIPE_{_lang}")
        pipe.return_value = mock.MagicMock(sentences=[])
        return pipe

    fake_stanza.Pipeline.side_effect = lambda lang, **kw: (
        _make_pipeline(lang) if lang != "su" else (_ for _ in ()).throw(
            ValueError("No processors to load for language su.")
        )
    )

    with _fake_runtime(fake_stanza):
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

        with pytest.raises(
            RuntimeError,
            match=r"failed to initialize Stanza pipeline.*langs=\['su'\]",
        ):
            backend.call(inputs)

        # A later batch reports the memoized failure loudly without trying
        # to construct the same known-broken pipeline again.
        with pytest.raises(
            RuntimeError,
            match=r"Stanza pipeline is unavailable.*langs=\['su'\]",
        ):
            backend.call(inputs[1:])

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

    with _fake_runtime(fake_stanza):
        a = StanzaBackend(lang="en,es", retokenize=False)
        b = StanzaBackend(lang="es,en", retokenize=False)

    assert a._nlp is mock.sentinel.PIPE_MULTI
    assert b._nlp is mock.sentinel.PIPE_MULTI
    assert fake_stanza.MultilingualPipeline.call_count == 1
    assert (
        fake_stanza.MultilingualPipeline.call_args.kwargs["download_method"]
        == "reuse_resources"
    )
    configs = fake_stanza.MultilingualPipeline.call_args.kwargs["lang_configs"]
    assert all(
        config["download_method"] == "reuse_resources"
        for config in configs.values()
    )
