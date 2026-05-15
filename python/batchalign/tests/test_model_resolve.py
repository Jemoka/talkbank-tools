"""Tests for ``batchalign.models.resolve`` — per-language model_id resolver.

The resolver is a nested dict keyed on model family then ISO-639-3 code.
These tests pin the entries we rely on in production — anyone removing or
renaming an entry must update both the resolver and any callers.
"""

from __future__ import annotations

from batchalign.models.resolve import resolve


class TestResolveUtterance:
    """Existing utterance-segmentation entries — guard rails for regressions."""

    def test_utterance_eng_returns_chatutterance_en(self) -> None:
        assert resolve("utterance", "eng") == "talkbank/CHATUtterance-en"

    def test_utterance_zho_returns_chatutterance_zh_cn(self) -> None:
        assert resolve("utterance", "zho") == "talkbank/CHATUtterance-zh_CN"

    def test_utterance_yue_returns_cantonese_model(self) -> None:
        assert (
            resolve("utterance", "yue")
            == "PolyU-AngelChanLab/Cantonese-Utterance-Segmentation"
        )

    def test_utterance_cmn_returns_chatutterance_zh_cn(self) -> None:
        assert resolve("utterance", "cmn") == "talkbank/CHATUtterance-zh_CN"

    def test_utterance_unknown_lang_returns_none(self) -> None:
        assert resolve("utterance", "xyz") is None


class TestResolveWhisperHub:
    """WhisperHub ASR fine-tune model_id resolution.

    The table seeds `mal` because an empirical evaluation showed that
    `thennal/whisper-medium-ml` is the only path that produces coherent
    Malayalam — stock Whisper and Rev.AI both fail for that language.
    See `book/src/reference/whisper-hub-asr.md` for the comparison.
    """

    def test_whisper_hub_malayalam_returns_thennal_medium_ml(self) -> None:
        # RED — "whisper_hub" family is not yet seeded in _RESOLVER.
        assert resolve("whisper_hub", "mal") == "thennal/whisper-medium-ml"

    def test_whisper_hub_unseeded_language_returns_none(self) -> None:
        # The resolver must return None for languages we haven't
        # characterized. Callers are responsible for surfacing a typed
        # error to the user with a clear "no default for this language,
        # pass a model_id" message.
        assert resolve("whisper_hub", "eng") is None

    def test_unknown_family_returns_none(self) -> None:
        assert resolve("nonexistent_family", "mal") is None
