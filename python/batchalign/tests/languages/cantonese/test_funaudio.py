"""Unit tests for _funaudio_common.py — FunAudioRecognizer logic."""

from __future__ import annotations

import builtins
import sys
import types

import pytest

from batchalign.inference.languages.cantonese._funaudio_common import FunAsrSegment, FunAudioRecognizer


_S = FunAsrSegment


# ---------------------------------------------------------------------------
# _clean_segment_text
# ---------------------------------------------------------------------------


class TestCleanSegmentText:
    def test_strips_markers(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("<|zh|> hello") == "hello"

    def test_strips_cjk_punctuation(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("你好，世界！") == "你好 世界"

    def test_strips_brackets(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("「hello」") == "hello"

    def test_complex_real_output(self) -> None:
        assert (
            FunAudioRecognizer._clean_segment_text("<|zh|> 「你好」，我係啊！")
            == "你好 我係啊"
        )

    def test_multiple_markers(self) -> None:
        assert (
            FunAudioRecognizer._clean_segment_text(
                "<|zh|><|HAPPY|> hello <|NEUTRAL|> world"
            )
            == "hello world"
        )

    def test_empty_string(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("") == ""

    def test_only_punctuation(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("，。！？") == ""

    def test_question_mark(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("係咪？") == "係咪"

    def test_period(self) -> None:
        assert FunAudioRecognizer._clean_segment_text("好似。") == "好似"


# ---------------------------------------------------------------------------
# FunAsrSegment.from_raw
# ---------------------------------------------------------------------------


class TestFunAsrSegmentFromRaw:
    def test_parses_normal_segment(self) -> None:
        seg = _S.from_raw({"text": "hello", "timestamp": [[0, 100]]})
        assert seg.text == "hello"
        assert seg.timestamp == [[0, 100]]

    def test_missing_text_defaults_empty(self) -> None:
        seg = _S.from_raw({})
        assert seg.text == ""
        assert seg.timestamp == []

    def test_non_list_timestamp_defaults_empty(self) -> None:
        seg = _S.from_raw({"text": "hi", "timestamp": "bad"})
        assert seg.timestamp == []

    def test_missing_timestamp_defaults_empty(self) -> None:
        seg = _S.from_raw({"text": "hi"})
        assert seg.timestamp == []


# ---------------------------------------------------------------------------
# transcribe — with stubbed _run_model
# ---------------------------------------------------------------------------


class TestTranscribe:
    @staticmethod
    def _make_recognizer(lang: str = "yue") -> FunAudioRecognizer:
        return FunAudioRecognizer(lang=lang)

    def test_cantonese_char_tokenization(self) -> None:
        rec = self._make_recognizer("yue")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(
                text="<|zh|> 我仲清楚啊",
                timestamp=[[0, 100], [100, 200], [200, 300], [300, 400], [400, 500]],
            )
        ]
        payload, timed_words = rec.transcribe("dummy.wav")
        values = [el["value"] for el in payload["monologues"][0]["elements"]]
        assert values == ["我", "仲", "清", "楚", "啊"]
        assert len(timed_words) == 5

    def test_cantonese_normalization_in_transcribe(self) -> None:
        rec = self._make_recognizer("yue")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|zh|> 真系", timestamp=[[0, 100], [100, 200]])
        ]
        payload, _ = rec.transcribe("dummy.wav")
        values = [el["value"] for el in payload["monologues"][0]["elements"]]
        assert values == ["真", "係"]

    def test_english_word_tokenization(self) -> None:
        rec = self._make_recognizer("eng")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|en|> hello world", timestamp=[[0, 500], [600, 1000]])
        ]
        payload, timed_words = rec.transcribe("dummy.wav")
        elements = payload["monologues"][0]["elements"]
        assert [el["value"] for el in elements] == ["hello", "world"]
        assert len(timed_words) == 2

    def test_timed_words_sorted(self) -> None:
        rec = self._make_recognizer("eng")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|en|> hello world", timestamp=[[500, 800], [100, 400]])
        ]
        _, timed_words = rec.transcribe("dummy.wav")
        starts = [tw["start_ms"] for tw in timed_words]
        assert starts == [100, 500]

    def test_missing_timestamps(self) -> None:
        rec = self._make_recognizer("eng")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|en|> hello world bye", timestamp=[[0, 200]])
        ]
        payload, timed_words = rec.transcribe("dummy.wav")
        elements = payload["monologues"][0]["elements"]
        assert len(elements) == 3
        assert elements[0]["ts"] == 0.0
        assert elements[1]["ts"] is None
        assert elements[2]["ts"] is None
        assert len(timed_words) == 1

    def test_no_timestamp_key(self) -> None:
        rec = self._make_recognizer("eng")
        rec._run_model = lambda _path: [_S(text="<|en|> hello")]  # type: ignore[method-assign]
        payload, timed_words = rec.transcribe("dummy.wav")
        assert len(payload["monologues"]) == 1
        assert payload["monologues"][0]["elements"][0]["ts"] is None
        assert timed_words == []

    def test_multiple_segments(self) -> None:
        rec = self._make_recognizer("eng")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|en|> hello", timestamp=[[0, 200]]),
            _S(text="<|en|> world", timestamp=[[500, 700]]),
        ]
        payload, _ = rec.transcribe("dummy.wav")
        assert len(payload["monologues"]) == 2

    def test_empty_model_output(self) -> None:
        rec = self._make_recognizer("yue")
        rec._run_model = lambda _path: []  # type: ignore[method-assign]
        payload, timed_words = rec.transcribe("dummy.wav")
        assert payload["monologues"] == []
        assert timed_words == []

    def test_zero_duration_word_excluded_from_timed(self) -> None:
        rec = self._make_recognizer("eng")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|en|> hello", timestamp=[[100, 100]])
        ]
        payload, timed_words = rec.transcribe("dummy.wav")
        assert len(payload["monologues"][0]["elements"]) == 1
        assert timed_words == []

    def test_speaker_always_zero(self) -> None:
        rec = self._make_recognizer("yue")
        rec._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|zh|> 好", timestamp=[[0, 100]])
        ]
        payload, _ = rec.transcribe("dummy.wav")
        assert payload["monologues"][0]["speaker"] == 0


class TestProtocolSafety:
    def test_get_model_reports_missing_dependency(self, monkeypatch) -> None:
        original_import = builtins.__import__

        def fake_import(name, *args, **kwargs):
            if name == "funasr":
                raise ImportError("missing")
            return original_import(name, *args, **kwargs)

        monkeypatch.setattr(builtins, "__import__", fake_import)

        with pytest.raises(ImportError, match="funasr"):
            FunAudioRecognizer(lang="yue")._get_model()

    def test_get_model_suppresses_funasr_stdout(self, monkeypatch, capsys) -> None:
        fake_module = types.ModuleType("funasr")

        class FakeAutoModel:
            def __init__(self, **_kwargs) -> None:
                print("funasr version: 1.3.1.")

        fake_module.AutoModel = FakeAutoModel
        monkeypatch.setitem(sys.modules, "funasr", fake_module)

        rec = FunAudioRecognizer(lang="yue")
        rec._get_model()

        captured = capsys.readouterr()
        assert captured.out == ""

    def test_get_model_uses_paraformer_constructor(self, monkeypatch) -> None:
        fake_module = types.ModuleType("funasr")
        seen: dict[str, object] = {}

        class FakeAutoModel:
            def __init__(self, **kwargs) -> None:
                seen.update(kwargs)

        fake_module.AutoModel = FakeAutoModel
        monkeypatch.setitem(sys.modules, "funasr", fake_module)

        rec = FunAudioRecognizer(lang="yue", model="paraformer-zh", device="cuda")
        rec._get_model()

        assert seen["model"] == "paraformer-zh"
        assert seen["model_revision"] == "v2.0.4"
        assert seen["vad_model_revision"] == "v2.0.4"
        assert seen["punc_model"] == "ct-punc-c"

    def test_run_model_suppresses_generate_stdout(self, capsys) -> None:
        class FakeModel:
            def generate(self, **_kwargs):
                print("funasr version: 1.3.1.")
                return [{"text": "<|zh|> 好", "timestamp": [[0, 100]]}]

        rec = FunAudioRecognizer(lang="yue")
        rec._model = FakeModel()

        segments = rec._run_model("dummy.wav")

        captured = capsys.readouterr()
        assert captured.out == ""
        assert segments == [_S(text="<|zh|> 好", timestamp=[[0, 100]])]

    def test_run_model_accepts_paraformer_dict_output(self) -> None:
        class FakeModel:
            def __init__(self) -> None:
                self.calls: list[dict[str, object]] = []

            def generate(self, **kwargs):
                self.calls.append(kwargs)
                return {"text": "<|zh|> 好", "timestamp": [[0, 100]]}

        rec = FunAudioRecognizer(lang="yue", model="paraformer-zh")
        rec._model = fake_model = FakeModel()

        segments = rec._run_model("dummy.wav")

        assert fake_model.calls == [{"input": "dummy.wav", "output_timestamp": True}]
        assert segments == [_S(text="<|zh|> 好", timestamp=[[0, 100]])]

    def test_run_model_ignores_non_collection_output(self) -> None:
        class FakeModel:
            def generate(self, **_kwargs):
                return "not-a-segment-list"

        rec = FunAudioRecognizer(lang="yue")
        rec._model = FakeModel()

        assert rec._run_model("dummy.wav") == []
