"""Unit tests for _common.py — Cantonese normalization, config, timestamps."""

from __future__ import annotations

import configparser

import pytest

from batchalign.inference.languages.cantonese._common import (
    normalize_cantonese_char_tokens,
    normalize_cantonese_text,
    normalize_cantonese_token,
    parse_timestamp_pair,
    provider_lang_code,
    read_asr_config,
)
from batchalign.errors import ConfigError


# ---------------------------------------------------------------------------
# Cantonese normalization — real examples from 05b.cha
# ---------------------------------------------------------------------------


class TestNormalizeCantoneseText:
    def test_single_char_replacement_xi_to_hai(self) -> None:
        assert normalize_cantonese_text("系") == "係"

    def test_multi_char_before_single_char(self) -> None:
        assert normalize_cantonese_text("真系") == "真係"

    def test_combined_replacements(self) -> None:
        assert normalize_cantonese_text("系呀") == "係啊"

    def test_no_replacement_needed(self) -> None:
        assert normalize_cantonese_text("你好") == "你好"

    def test_empty_string(self) -> None:
        assert normalize_cantonese_text("") == ""

    @pytest.mark.parametrize(
        "src,expected",
        [
            ("繫", "係"),
            ("呀", "啊"),
            ("噶", "㗎"),
            ("咧", "呢"),
            ("嗬", "喎"),
            ("只", "隻"),
            ("咯", "囉"),
            ("嚇", "吓"),
            ("啫", "咋"),
            ("哇", "嘩"),
            ("着", "著"),
            ("嘞", "喇"),
            ("啵", "噃"),
            ("松", "鬆"),
            ("吵", "嘈"),
        ],
    )
    def test_single_char_replacements(self, src: str, expected: str) -> None:
        assert normalize_cantonese_text(src) == expected

    @pytest.mark.parametrize(
        "src,expected",
        [
            ("真系", "真係"),
            ("唔系", "唔係"),
            ("中意", "鍾意"),
            ("遊水", "游水"),
            ("古仔", "故仔"),
            ("較剪", "鉸剪"),
            ("衝涼", "沖涼"),
            ("分鍾", "分鐘"),
            ("重復", "重複"),
        ],
    )
    def test_multi_char_replacements(self, src: str, expected: str) -> None:
        assert normalize_cantonese_text(src) == expected

    def test_real_utterance_from_corpus(self) -> None:
        assert normalize_cantonese_text("系") == "係"

    def test_multi_replacement_in_sentence(self) -> None:
        assert normalize_cantonese_text("你真系好吵呀") == "你真係好嘈啊"


class TestNormalizeCantoneseToken:
    def test_yue_applies_normalization(self) -> None:
        assert normalize_cantonese_token("系", "yue") == "係"

    def test_non_yue_passthrough(self) -> None:
        assert normalize_cantonese_token("系", "zho") == "系"
        assert normalize_cantonese_token("hello", "eng") == "hello"

    def test_yue_no_change_needed(self) -> None:
        assert normalize_cantonese_token("係", "yue") == "係"


class TestNormalizeCantoneseCharTokens:
    def test_strips_punctuation(self) -> None:
        assert normalize_cantonese_char_tokens("真系呀，") == ["真", "係", "啊"]

    def test_strips_all_punctuation_types(self) -> None:
        assert normalize_cantonese_char_tokens("你。好，啊！呢？「吓」") == [
            "你", "好", "啊", "呢", "吓",
        ]

    def test_empty_string(self) -> None:
        assert normalize_cantonese_char_tokens("") == []

    def test_only_punctuation(self) -> None:
        assert normalize_cantonese_char_tokens("，。！") == []

    def test_spaces_stripped(self) -> None:
        assert normalize_cantonese_char_tokens("你 好") == ["你", "好"]

    def test_real_corpus_phrase(self) -> None:
        assert normalize_cantonese_char_tokens("咁搞笑嘅") == ["咁", "搞", "笑", "嘅"]


# ---------------------------------------------------------------------------
# provider_lang_code
# ---------------------------------------------------------------------------


class TestProviderLangCode:
    def test_yue_passthrough(self) -> None:
        assert provider_lang_code("yue") == "yue"

    def test_iso3_to_iso2(self) -> None:
        assert provider_lang_code("eng") == "en"
        assert provider_lang_code("fra") == "fr"
        assert provider_lang_code("zho") == "zh"
        assert provider_lang_code("jpn") == "ja"

    def test_unknown_passthrough(self) -> None:
        assert provider_lang_code("zzz") == "zzz"
        assert provider_lang_code("xyz") == "xyz"

    def test_pycountry_errors_fall_back_to_original_code(self, monkeypatch) -> None:
        monkeypatch.setattr(
            "batchalign.inference.languages.cantonese._common.pycountry.languages.get",
            lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("boom")),
        )
        assert provider_lang_code("eng") == "eng"


# ---------------------------------------------------------------------------
# parse_timestamp_pair
# ---------------------------------------------------------------------------


class TestParseTimestampPair:
    def test_normal_ints(self) -> None:
        assert parse_timestamp_pair([100, 200]) == (100, 200)

    def test_float_rounding(self) -> None:
        assert parse_timestamp_pair([100.4, 210.6]) == (100, 211)

    def test_none_input(self) -> None:
        assert parse_timestamp_pair(None) == (None, None)

    def test_non_numeric(self) -> None:
        assert parse_timestamp_pair(["x", "y"]) == (None, None)

    def test_short_list(self) -> None:
        assert parse_timestamp_pair([100]) == (None, None)

    def test_empty_list(self) -> None:
        assert parse_timestamp_pair([]) == (None, None)

    def test_string_input(self) -> None:
        assert parse_timestamp_pair("ab") == (None, None)

    def test_tuple_input(self) -> None:
        assert parse_timestamp_pair((50, 100)) == (50, 100)

    def test_zero_values(self) -> None:
        assert parse_timestamp_pair([0, 0]) == (0, 0)

    def test_large_values(self) -> None:
        assert parse_timestamp_pair([3600000, 3600500]) == (3600000, 3600500)


# ---------------------------------------------------------------------------
# read_asr_config
# ---------------------------------------------------------------------------


def config_with_asr(**entries: str) -> configparser.ConfigParser:
    """Build a minimal `[asr]` config fixture for HK config tests."""
    cfg = configparser.ConfigParser()
    cfg.add_section("asr")
    for key, value in entries.items():
        cfg.set("asr", key, value)
    return cfg


class TestReadAsrConfig:
    def test_missing_asr_section(self) -> None:
        with pytest.raises(ConfigError):
            read_asr_config(
                ("engine.tencent.id",),
                engine="Tencent",
                config=configparser.ConfigParser(),
            )

    def test_missing_keys(self) -> None:
        with pytest.raises(ConfigError):
            read_asr_config(
                ("engine.tencent.id", "engine.tencent.key"),
                engine="Tencent",
                config=config_with_asr(),
            )

    def test_empty_value(self) -> None:
        with pytest.raises(ConfigError):
            read_asr_config(
                ("engine.tencent.id",),
                engine="Tencent",
                config=config_with_asr(**{"engine.tencent.id": "   "}),
            )

    def test_valid_config(self) -> None:
        values = read_asr_config(
            ("engine.tencent.id", "engine.tencent.key"),
            engine="Tencent",
            config=config_with_asr(
                **{
                    "engine.tencent.id": " my_id ",
                    "engine.tencent.key": " my_key ",
                }
            ),
        )
        assert values["engine.tencent.id"] == "my_id"
        assert values["engine.tencent.key"] == "my_key"

    def test_injected_env_overrides_config_file_reads(self) -> None:
        values = read_asr_config(
            ("engine.tencent.id", "engine.tencent.key"),
            engine="Tencent",
            config=configparser.ConfigParser(),
            environ={
                "BATCHALIGN_TENCENT_ID": " env-id ",
                "BATCHALIGN_TENCENT_KEY": " env-key ",
            },
        )
        assert values["engine.tencent.id"] == "env-id"
        assert values["engine.tencent.key"] == "env-key"

    def test_aliyun_keys(self) -> None:
        values = read_asr_config(
            ("engine.aliyun.ak_id", "engine.aliyun.ak_secret", "engine.aliyun.ak_appkey"),
            engine="Aliyun",
            config=config_with_asr(
                **{
                    "engine.aliyun.ak_id": "ak_id_val",
                    "engine.aliyun.ak_secret": "ak_secret_val",
                    "engine.aliyun.ak_appkey": "ak_appkey_val",
                }
            ),
        )
        assert len(values) == 3
        assert values["engine.aliyun.ak_id"] == "ak_id_val"
