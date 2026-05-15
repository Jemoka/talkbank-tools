"""Smoke tests for HK engine helper/adaptor logic."""

from __future__ import annotations

import configparser
from types import SimpleNamespace

import pytest

from batchalign.inference.languages.cantonese._common import (
    normalize_cantonese_char_tokens,
    parse_timestamp_pair,
    provider_lang_code,
    read_asr_config,
)
from batchalign.inference.languages.cantonese._funaudio_common import FunAsrSegment, FunAudioRecognizer
from batchalign.inference.languages.cantonese._tencent_api import TencentRecognizer
from batchalign.errors import ConfigError


def config_with_asr(**entries: str) -> configparser.ConfigParser:
    """Build a minimal `[asr]` config fixture for helper-level tests."""
    cfg = configparser.ConfigParser()
    cfg.add_section("asr")
    for key, value in entries.items():
        cfg.set("asr", key, value)
    return cfg


class TestHKHelpers:
    def test_parse_timestamp_pair(self) -> None:
        assert parse_timestamp_pair([100.4, 210.6]) == (100, 211)
        assert parse_timestamp_pair(None) == (None, None)
        assert parse_timestamp_pair(["x", "y"]) == (None, None)

    def test_normalize_cantonese_char_tokens(self) -> None:
        assert normalize_cantonese_char_tokens("真系呀，") == ["真", "係", "啊"]

    def test_read_asr_config_validation(self) -> None:
        with pytest.raises(ConfigError):
            read_asr_config(
                ("engine.tencent.id",),
                engine="Tencent",
                config=configparser.ConfigParser(),
            )

        with pytest.raises(ConfigError):
            read_asr_config(
                ("engine.tencent.id", "engine.tencent.key"),
                engine="Tencent",
                config=config_with_asr(),
            )

        values = read_asr_config(
            ("engine.tencent.id", "engine.tencent.key"),
            engine="Tencent",
            config=config_with_asr(
                **{
                    "engine.tencent.id": " id ",
                    "engine.tencent.key": " key ",
                }
            ),
        )
        assert values["engine.tencent.id"] == "id"
        assert values["engine.tencent.key"] == "key"

    def test_provider_lang_code(self) -> None:
        assert provider_lang_code("yue") == "yue"
        assert provider_lang_code("eng") == "en"
        assert provider_lang_code("zzz") == "zzz"

    def test_funaudio_clean_segment_text(self) -> None:
        cleaned = FunAudioRecognizer._clean_segment_text("<|zh|> 「hello」，world！")
        assert cleaned == "hello world"

    def test_funaudio_transcribe_sorts_timed_words(self) -> None:
        recognizer = FunAudioRecognizer(lang="eng")
        _S = FunAsrSegment
        recognizer._run_model = lambda _path: [  # type: ignore[method-assign]
            _S(text="<|zh|> hello， world！", timestamp=[[100, 200], [0, 50]])
        ]

        payload, timed_words = recognizer.transcribe("dummy.wav")
        assert [el["value"] for el in payload["monologues"][0]["elements"]] == [
            "hello",
            "world",
        ]
        assert [(tw["word"], tw["start_ms"], tw["end_ms"]) for tw in timed_words] == [
            ("world", 0, 50),
            ("hello", 100, 200),
        ]

    def test_tencent_monologues_and_timed_words(self) -> None:
        recognizer = TencentRecognizer.__new__(TencentRecognizer)
        recognizer.lang_code = "yue"
        recognizer.provider_lang = "yue"

        result_detail = [
            SimpleNamespace(
                StartMs=1000,
                SpeakerId=2,
                Words=[
                    SimpleNamespace(Word="系", OffsetStartMs=0, OffsetEndMs=200),
                    SimpleNamespace(Word="你", OffsetStartMs=300, OffsetEndMs=500),
                ],
            ),
            SimpleNamespace(
                StartMs=500,
                SpeakerId=1,
                Words=[
                    SimpleNamespace(Word="呀", OffsetStartMs=0, OffsetEndMs=100),
                ],
            ),
        ]

        payload = recognizer.monologues(result_detail)
        assert len(payload["monologues"]) == 2
        first_elements = payload["monologues"][0]["elements"]
        assert first_elements[0]["value"] == "係"
        assert first_elements[1]["value"] == "你"

        timed = recognizer.timed_words(result_detail)
        assert [(item["word"], item["start_ms"], item["end_ms"]) for item in timed] == [
            ("啊", 500, 600),
            ("係", 1000, 1200),
            ("你", 1300, 1500),
        ]
