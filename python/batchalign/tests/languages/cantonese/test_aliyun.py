# affects: python/batchalign/inference/languages/cantonese/_aliyun_asr.py
"""Unit tests for _aliyun_asr.py — AliyunRunner sentence parsing and load validation."""

from __future__ import annotations

import json

import pytest

from batchalign.inference.languages.cantonese._aliyun_asr import (
    _AliyunRunner,
    _project_results,
    AliyunSentenceResult,
    AliyunWord,
    load_aliyun_asr,
)


# ---------------------------------------------------------------------------
# _AliyunRunner sentence parsing (via Pydantic models)
# ---------------------------------------------------------------------------


class TestAliyunRunnerSentenceParsing:
    @staticmethod
    def _make_runner() -> _AliyunRunner:
        return _AliyunRunner(token="fake", appkey="fake", wav_path="fake.wav")

    def test_on_sentence_end_parses_words(self) -> None:
        runner = self._make_runner()
        message = json.dumps(
            {
                "payload": {
                    "result": "你好",
                    "words": [
                        {"text": "你", "startTime": 0, "endTime": 200},
                        {"text": "好", "startTime": 300, "endTime": 500},
                    ],
                }
            }
        )
        runner._on_sentence_end(message)
        assert len(runner._results) == 1
        result = runner._results[0]
        assert len(result.words) == 2
        assert result.words[0].text == "你"
        assert result.words[0].startTime == 0
        assert result.words[0].endTime == 200
        assert result.sentence_text == "你好"

    def test_on_sentence_end_empty_words(self) -> None:
        runner = self._make_runner()
        message = json.dumps({"payload": {"words": [], "result": "test"}})
        runner._on_sentence_end(message)
        assert len(runner._results) == 1
        assert runner._results[0].words == []
        assert runner._results[0].sentence_text == "test"

    def test_on_sentence_end_missing_words_key(self) -> None:
        runner = self._make_runner()
        message = json.dumps({"payload": {"result": "hello"}})
        runner._on_sentence_end(message)
        assert len(runner._results) == 1
        assert runner._results[0].words == []
        assert runner._results[0].sentence_text == "hello"

    def test_on_sentence_end_missing_payload(self) -> None:
        runner = self._make_runner()
        message = json.dumps({})
        runner._on_sentence_end(message)
        assert len(runner._results) == 1
        assert runner._results[0].words == []
        assert runner._results[0].sentence_text == ""

    def test_on_sentence_end_multiple_sentences(self) -> None:
        runner = self._make_runner()
        for text in ("你", "好"):
            message = json.dumps(
                {
                    "payload": {
                        "words": [{"text": text, "startTime": 0, "endTime": 100}],
                        "result": text,
                    }
                }
            )
            runner._on_sentence_end(message)
        assert len(runner._results) == 2
        assert runner._results[0].words[0].text == "你"
        assert runner._results[1].words[0].text == "好"

    def test_word_defaults(self) -> None:
        runner = self._make_runner()
        message = json.dumps(
            {"payload": {"words": [{"text": "test"}], "result": "test"}}
        )
        runner._on_sentence_end(message)
        word = runner._results[0].words[0]
        assert word.text == "test"
        assert word.startTime == 0
        assert word.endTime == 0

    def test_on_error_raises(self) -> None:
        runner = self._make_runner()
        with pytest.raises(RuntimeError, match="Aliyun ASR error"):
            runner._on_error("connection refused")

    def test_on_sentence_begin_noop(self) -> None:
        runner = self._make_runner()
        runner._on_sentence_begin("msg")

    def test_on_close_noop(self) -> None:
        runner = self._make_runner()
        runner._on_close()


# ---------------------------------------------------------------------------
# load_aliyun_asr validation
# ---------------------------------------------------------------------------


class TestLoadAliyunAsrValidation:
    def test_non_yue_raises(self) -> None:
        with pytest.raises(ValueError, match="yue"):
            load_aliyun_asr("eng", None)


class TestAliyunProjection:
    """Coverage for the Rust-owned Aliyun projection bridge."""

    def test_project_results_preserves_word_timings(self) -> None:
        response = _project_results(
            [
                AliyunSentenceResult(
                    words=[AliyunWord(text="你", startTime=0, endTime=200)],
                    sentence_text="你",
                )
            ]
        )

        assert response.kind == "monologues"
        assert response.lang == "yue"
        assert response.monologues[0].elements[0].value == "你"
        assert response.monologues[0].elements[0].ts == 0.0
        assert response.monologues[0].elements[0].end_ts == 0.2

    def test_project_results_tokenizes_sentence_fallback_in_rust(self) -> None:
        response = _project_results(
            [
                AliyunSentenceResult(
                    words=[],
                    sentence_text="真系呀，",
                )
            ]
        )

        assert [
            element.value for element in response.monologues[0].elements
        ] == ["真", "係", "啊"]
        assert all(
            element.ts is None and element.end_ts is None
            for element in response.monologues[0].elements
        )
