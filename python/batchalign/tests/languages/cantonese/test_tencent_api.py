"""Unit tests for _tencent_api.py — TencentRecognizer monologues/timed_words logic."""

from __future__ import annotations

from types import SimpleNamespace

from batchalign.inference.languages.cantonese._tencent_api import TencentRecognizer


def _make_recognizer(lang: str = "yue") -> TencentRecognizer:
    """Create a TencentRecognizer without calling __init__ (skips credential loading)."""
    rec = TencentRecognizer.__new__(TencentRecognizer)
    rec.lang_code = lang
    rec.provider_lang = lang if lang == "yue" else lang[:2]
    return rec


# ---------------------------------------------------------------------------
# _engine_model_type
# ---------------------------------------------------------------------------


class TestEngineModelType:
    def test_cantonese(self) -> None:
        rec = _make_recognizer("yue")
        assert rec._engine_model_type() == "16k_zh_large"

    def test_mandarin(self) -> None:
        rec = _make_recognizer("zho")
        assert rec._engine_model_type() == "16k_zh_large"

    def test_english(self) -> None:
        rec = _make_recognizer("eng")
        rec.provider_lang = "en"
        assert rec._engine_model_type() == "16k_en"

    def test_french(self) -> None:
        rec = _make_recognizer("fra")
        rec.provider_lang = "fr"
        assert rec._engine_model_type() == "16k_fr"


# ---------------------------------------------------------------------------
# monologues — using real-style data from 05b.cha
# ---------------------------------------------------------------------------


class TestMonologues:
    def test_basic_cantonese(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=4850,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="咁", OffsetStartMs=0, OffsetEndMs=200),
                    SimpleNamespace(Word="搞", OffsetStartMs=250, OffsetEndMs=500),
                    SimpleNamespace(Word="笑", OffsetStartMs=550, OffsetEndMs=800),
                    SimpleNamespace(Word="嘅", OffsetStartMs=850, OffsetEndMs=1025),
                ],
            ),
        ]
        payload = rec.monologues(result_detail)
        assert len(payload["monologues"]) == 1
        elements = payload["monologues"][0]["elements"]
        assert len(elements) == 4
        assert elements[0]["value"] == "咁"
        assert abs(elements[0]["ts"] - 4.850) < 0.001
        assert abs(elements[0]["end_ts"] - 5.050) < 0.001

    def test_normalization_applied(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=1000,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="系", OffsetStartMs=0, OffsetEndMs=200),
                ],
            ),
        ]
        payload = rec.monologues(result_detail)
        assert payload["monologues"][0]["elements"][0]["value"] == "係"

    def test_empty_result_detail(self) -> None:
        rec = _make_recognizer("yue")
        payload = rec.monologues([])
        assert payload["monologues"] == []

    def test_empty_words_skipped(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [SimpleNamespace(StartMs=0, SpeakerId=1, Words=[])]
        payload = rec.monologues(result_detail)
        assert payload["monologues"] == []

    def test_blank_word_skipped(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=0,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="  ", OffsetStartMs=0, OffsetEndMs=100),
                    SimpleNamespace(Word="好", OffsetStartMs=200, OffsetEndMs=300),
                ],
            ),
        ]
        payload = rec.monologues(result_detail)
        elements = payload["monologues"][0]["elements"]
        assert len(elements) == 1
        assert elements[0]["value"] == "好"

    def test_multi_speaker(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=6750,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="我", OffsetStartMs=0, OffsetEndMs=150),
                    SimpleNamespace(Word="仲", OffsetStartMs=200, OffsetEndMs=350),
                ],
            ),
            SimpleNamespace(
                StartMs=8930,
                SpeakerId=2,
                Words=[
                    SimpleNamespace(Word="我", OffsetStartMs=0, OffsetEndMs=200),
                ],
            ),
        ]
        payload = rec.monologues(result_detail)
        assert len(payload["monologues"]) == 2
        assert payload["monologues"][0]["speaker"] == 1
        assert payload["monologues"][1]["speaker"] == 2

    def test_missing_attributes_handled(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [SimpleNamespace()]
        payload = rec.monologues(result_detail)
        assert payload["monologues"] == []

    def test_english_no_normalization(self) -> None:
        rec = _make_recognizer("eng")
        rec.provider_lang = "en"
        result_detail = [
            SimpleNamespace(
                StartMs=0,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="hello", OffsetStartMs=0, OffsetEndMs=500),
                ],
            ),
        ]
        payload = rec.monologues(result_detail)
        assert payload["monologues"][0]["elements"][0]["value"] == "hello"


# ---------------------------------------------------------------------------
# timed_words
# ---------------------------------------------------------------------------


class TestTimedWords:
    def test_basic(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=1000,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="好", OffsetStartMs=0, OffsetEndMs=200),
                    SimpleNamespace(Word="似", OffsetStartMs=300, OffsetEndMs=450),
                ],
            ),
        ]
        timed = rec.timed_words(result_detail)
        assert len(timed) == 2
        assert timed[0]["word"] == "好"
        assert timed[0]["start_ms"] == 1000
        assert timed[0]["end_ms"] == 1200
        assert timed[1]["word"] == "似"
        assert timed[1]["start_ms"] == 1300
        assert timed[1]["end_ms"] == 1450

    def test_zero_duration_filtered(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=0,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="嗯", OffsetStartMs=100, OffsetEndMs=100),
                ],
            ),
        ]
        assert rec.timed_words(result_detail) == []

    def test_sorted_across_segments(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=5000,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="後", OffsetStartMs=0, OffsetEndMs=200),
                ],
            ),
            SimpleNamespace(
                StartMs=1000,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="前", OffsetStartMs=0, OffsetEndMs=200),
                ],
            ),
        ]
        timed = rec.timed_words(result_detail)
        assert timed[0]["word"] == "前"
        assert timed[1]["word"] == "後"

    def test_cantonese_normalization_in_timed_words(self) -> None:
        rec = _make_recognizer("yue")
        result_detail = [
            SimpleNamespace(
                StartMs=500,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="呀", OffsetStartMs=0, OffsetEndMs=100),
                ],
            ),
        ]
        timed = rec.timed_words(result_detail)
        assert timed[0]["word"] == "啊"

    def test_empty_input(self) -> None:
        rec = _make_recognizer("yue")
        assert rec.timed_words([]) == []
