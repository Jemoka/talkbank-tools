"""Worker utterance-model loading stays explicit per language code.

These tests capture the utterance-model wiring Spencer's question depends on:
Cantonese (`yue`) and both Mandarin codes (`cmn`, `zho`) load the dedicated
Mandarin/Cantonese boundary models at worker bootstrap.
"""

# affects: python/batchalign/models/resolve.py
# affects: python/batchalign/worker/_model_loading/utterance.py

from __future__ import annotations

from batchalign.worker._model_loading.utterance import load_utterance_model
from batchalign.worker._types import _state


def test_load_utterance_model_loads_zho_boundary_model(monkeypatch) -> None:
    captured: list[tuple[str, str | None]] = []
    old_model = _state.utterance_boundary_model
    old_name = _state.utterance_model_name

    try:
        class FakeBoundaryModel:
            def __init__(self, model_name: str, *, lang: str | None = None) -> None:
                captured.append((model_name, lang))
                self.model_name = model_name
                self.lang = lang

        monkeypatch.setattr(
            "batchalign.worker._model_loading.utterance.BertUtteranceModel",
            FakeBoundaryModel,
        )

        load_utterance_model("zho")

        assert captured == [("talkbank/CHATUtterance-zh_CN", "zho")]
        assert _state.utterance_model_name == "talkbank/CHATUtterance-zh_CN"
    finally:
        _state.utterance_boundary_model = old_model
        _state.utterance_model_name = old_name


def test_load_utterance_model_loads_yue_boundary_model(monkeypatch) -> None:
    captured: list[tuple[str, str | None]] = []
    old_model = _state.utterance_boundary_model
    old_name = _state.utterance_model_name

    try:
        class FakeBoundaryModel:
            def __init__(self, model_name: str, *, lang: str | None = None) -> None:
                captured.append((model_name, lang))
                self.model_name = model_name
                self.lang = lang

        monkeypatch.setattr(
            "batchalign.worker._model_loading.utterance.BertUtteranceModel",
            FakeBoundaryModel,
        )

        load_utterance_model("yue")

        assert captured == [
            ("PolyU-AngelChanLab/Cantonese-Utterance-Segmentation", "yue")
        ]
        assert (
            _state.utterance_model_name
            == "PolyU-AngelChanLab/Cantonese-Utterance-Segmentation"
        )
    finally:
        _state.utterance_boundary_model = old_model
        _state.utterance_model_name = old_name


def test_load_utterance_model_loads_cmn_boundary_model(monkeypatch) -> None:
    captured: list[tuple[str, str | None]] = []
    old_model = _state.utterance_boundary_model
    old_name = _state.utterance_model_name

    try:
        class FakeBoundaryModel:
            def __init__(self, model_name: str, *, lang: str | None = None) -> None:
                captured.append((model_name, lang))
                self.model_name = model_name
                self.lang = lang

        monkeypatch.setattr(
            "batchalign.worker._model_loading.utterance.BertUtteranceModel",
            FakeBoundaryModel,
        )

        load_utterance_model("cmn")

        assert captured == [("talkbank/CHATUtterance-zh_CN", "cmn")]
        assert _state.utterance_model_name == "talkbank/CHATUtterance-zh_CN"
    finally:
        _state.utterance_boundary_model = old_model
        _state.utterance_model_name = old_name
