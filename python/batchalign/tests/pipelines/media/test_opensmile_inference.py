"""Tests for the thin Python openSMILE inference boundary."""

from __future__ import annotations

from types import ModuleType, SimpleNamespace

import numpy as np

from batchalign.inference.opensmile import (
    OpenSmileBatchItem,
    OpenSmileResponse,
    _build_response,
    _build_smile_engine,
    _get_feature_set_map,
    _rows_from_dataframe,
    extract_features,
    infer_opensmile_item,
    infer_opensmile_prepared_audio,
)


class _FakeFrame:
    """Small dataframe-like double for openSMILE adapter tests."""

    def __init__(self, records: list[dict[str, float]]) -> None:
        self._records = records
        self.columns = list(records[0].keys()) if records else []
        self.empty = not records

    def to_dict(self, *, orient: str) -> list[dict[str, float]]:
        assert orient == "records"
        return self._records

    def __len__(self) -> int:
        return len(self._records)


def test_feature_set_map_caches_and_build_smile_engine_normalizes_level(monkeypatch) -> None:
    """Feature-set lookup should cache once and normalize non-functional levels."""

    captured: list[dict[str, object]] = []
    fake_opensmile = ModuleType("opensmile")
    fake_opensmile.FeatureSet = SimpleNamespace(
        eGeMAPSv02="egemaps-v02",
        eGeMAPSv01b="egemaps-v01b",
        GeMAPSv01b="gemaps-v01b",
        ComParE_2016="compare-2016",
    )
    fake_opensmile.FeatureLevel = SimpleNamespace(
        Functionals="functionals-enum",
        LowLevelDescriptors="lld-enum",
    )

    class _FakeSmile:
        def __init__(self, *, feature_set, feature_level) -> None:
            captured.append(
                {"feature_set": feature_set, "feature_level": feature_level}
            )

    fake_opensmile.Smile = _FakeSmile

    monkeypatch.setitem(__import__("sys").modules, "opensmile", fake_opensmile)
    monkeypatch.setattr("batchalign.inference.opensmile._FEATURE_SET_MAP", None)

    feature_map = _get_feature_set_map()
    smile, normalized_level = _build_smile_engine("unknown", "lld")

    assert feature_map["eGeMAPSv02"] == "egemaps-v02"
    assert normalized_level == "low_level_descriptors"
    assert isinstance(smile, _FakeSmile)
    assert captured == [
        {
            "feature_set": "egemaps-v02",
            "feature_level": "lld-enum",
        }
    ]


def test_rows_and_build_response_convert_dataframe_shapes() -> None:
    """The adapter should convert tabular output into JSON-safe float rows."""

    frame = _FakeFrame([{"f0": 1, "jitter": 2.5}, {"f0": 3.0, "jitter": 4}])

    rows = _rows_from_dataframe(frame)
    response = _build_response(
        audio_label="sample.wav",
        feature_set="eGeMAPSv02",
        feature_level="functionals",
        features_df=frame,
    )

    assert rows == [
        {"f0": 1.0, "jitter": 2.5},
        {"f0": 3.0, "jitter": 4.0},
    ]
    assert response.rows == rows
    assert response.num_features == 2
    assert response.duration_segments == 2
    assert response.success is True


def test_build_response_rejects_empty_results() -> None:
    """Empty openSMILE frames should fail before they reach worker serialization."""

    try:
        _build_response(
            audio_label="empty.wav",
            feature_set="eGeMAPSv02",
            feature_level="functionals",
            features_df=_FakeFrame([]),
        )
    except ValueError as error:
        assert "empty results" in str(error)
    else:  # pragma: no cover - explicit failure path
        raise AssertionError("expected ValueError for empty features")


def test_infer_opensmile_item_and_extract_features_wrap_engine_results(monkeypatch) -> None:
    """File-path inference should normalize levels and expose feature samples."""

    class _FakeSmile:
        def process_file(self, path: str) -> _FakeFrame:
            assert path == "/tmp/input.wav"
            return _FakeFrame([{"f0": 110.0, "jitter": 0.4}])

    monkeypatch.setattr(
        "batchalign.inference.opensmile._build_smile_engine",
        lambda feature_set, feature_level: (_FakeSmile(), "low_level_descriptors"),
    )

    response = infer_opensmile_item(
        OpenSmileBatchItem(
            audio_path="/tmp/input.wav",
            feature_set="eGeMAPSv02",
            feature_level="lld",
        )
    )
    payload = extract_features("/tmp/input.wav", feature_level="lld")

    assert response.feature_level == "low_level_descriptors"
    assert response.rows == [{"f0": 110.0, "jitter": 0.4}]
    assert payload["features_sample"] == {"f0": 110.0, "jitter": 0.4}


def test_infer_opensmile_prepared_audio_success_path(monkeypatch) -> None:
    """Prepared-audio openSMILE should normalize audio dtype and shape the response."""

    captured: dict[str, object] = {}

    class _FakeSmile:
        def process_signal(self, audio: np.ndarray, *, sampling_rate: int) -> _FakeFrame:
            captured["dtype"] = audio.dtype
            captured["sampling_rate"] = sampling_rate
            captured["values"] = audio.tolist()
            return _FakeFrame([{"f0": 220.0, "jitter": 0.7}])

    monkeypatch.setattr(
        "batchalign.inference.opensmile._build_smile_engine",
        lambda feature_set, feature_level: (_FakeSmile(), "functionals"),
    )

    response = infer_opensmile_prepared_audio(
        np.asarray([0.1, 0.2], dtype=np.float64),
        8000,
        audio_label="prepared.wav",
    )

    assert captured == {
        "dtype": np.float32,
        "sampling_rate": 8000,
        "values": [0.10000000149011612, 0.20000000298023224],
    }
    assert response.rows == [{"f0": 220.0, "jitter": 0.7}]
    assert response.audio_file == "prepared.wav"
    assert response.success is True


def test_infer_opensmile_item_and_prepared_audio_return_typed_errors(monkeypatch) -> None:
    """Both file-path and prepared-audio paths should surface extraction failures."""

    class _ExplodingSmile:
        def process_file(self, _path: str) -> _FakeFrame:
            raise RuntimeError("file extraction exploded")

        def process_signal(self, _audio: np.ndarray, *, sampling_rate: int) -> _FakeFrame:
            assert sampling_rate == 16000
            raise RuntimeError("signal extraction exploded")

    monkeypatch.setattr(
        "batchalign.inference.opensmile._build_smile_engine",
        lambda feature_set, feature_level: (_ExplodingSmile(), "functionals"),
    )

    file_response = infer_opensmile_item(
        OpenSmileBatchItem(audio_path="/tmp/bad.wav")
    )
    prepared_response = infer_opensmile_prepared_audio(
        np.asarray([0.1, 0.2], dtype=np.float64),
        16000,
        audio_label="prepared.wav",
    )

    assert file_response == OpenSmileResponse(
        feature_set="eGeMAPSv02",
        feature_level="functionals",
        num_features=0,
        duration_segments=0,
        audio_file="/tmp/bad.wav",
        rows=[],
        success=False,
        error="file extraction exploded",
    )
    assert prepared_response.success is False
    assert prepared_response.audio_file == "prepared.wav"
    assert prepared_response.error == "signal extraction exploded"
