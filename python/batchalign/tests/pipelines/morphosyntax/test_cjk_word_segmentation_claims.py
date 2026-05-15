"""Verification of CJK word segmentation assumptions.

This test module proves or disproves the specific assumptions that motivated
the ``--retokenize`` word segmentation feature for Cantonese and Mandarin:

1. FunASR/SenseVoice Cantonese output is per-character (no word boundaries)
2. Tencent ASR preserves multi-character word boundaries
3. PyCantonese ``segment()`` produces linguistically correct word boundaries
4. Stanza's Chinese tokenizer can segment Mandarin text into words

Each test documents the assumption being verified and uses real library calls
(PyCantonese, batchalign_core Rust functions), not synthetic doubles.
"""

from __future__ import annotations

import pytest


# =============================================================================
# Assumption 1: FunASR/SenseVoice outputs per-character tokens for Cantonese
# =============================================================================
#
# batchalign3's FunASR bridge (`_funaudio_common.py`) calls
# `cantonese_char_tokens()` in Rust, which splits text into per-character tokens.
# This is by design — FunASR returns raw text, and batchalign3 splits into
# characters for timestamp alignment. The result is per-character main tier words.
# =============================================================================


class TestClaim1_FunASR_PerCharacter:
    """FunASR Cantonese output is per-character — no word boundaries."""

    def test_funasr_cantonese_produces_per_char_tokens(self) -> None:
        """FunASR Cantonese transcription splits into individual characters.

        The Rust bridge `cantonese_char_tokens()` normalizes and splits CJK text
        into one token per character. This is the root of the word segmentation
        problem that --retokenize addresses.
        """
        import batchalign_core

        # Simulates FunASR output: "故事係好" (a story is good)
        tokens = batchalign_core.cantonese_char_tokens("故事係好")
        assert tokens == ["故", "事", "係", "好"], (
            "Each CJK character should be a separate token — "
            "this is the per-character problem that --retokenize solves"
        )

    def test_funasr_multichar_word_is_split(self) -> None:
        """Multi-character Cantonese words are split into individual characters.

        '故事' (story) is one word but FunASR processing splits it into '故' and '事'.
        This makes word count, MLU, and POS tagging unreliable.
        """
        import batchalign_core

        tokens = batchalign_core.cantonese_char_tokens("故事")
        assert len(tokens) == 2, "A two-character word should become 2 tokens"
        assert tokens == ["故", "事"]

    def test_funasr_sentence_all_single_chars(self) -> None:
        """A realistic Cantonese sentence produces only single-char tokens.

        '我想食嘢' (I want to eat something) — 4 characters, should be 2-3 words
        but FunASR produces 4 single-char tokens.
        """
        import batchalign_core

        tokens = batchalign_core.cantonese_char_tokens("我想食嘢")
        assert all(len(t) == 1 for t in tokens), (
            "Every token should be a single character — "
            "this is the per-character problem"
        )
        assert len(tokens) == 4


# =============================================================================
# Assumption 2: Tencent ASR returns word-segmented output
# =============================================================================
#
# Tencent's `ResultDetail` API returns a `Words` array where each entry is a
# pre-segmented word with its own timing. The Rust bridge
# `tencent_result_detail_to_asr()` preserves these word boundaries.
#
# We cannot call the Tencent API in unit tests (requires credentials).
# What we CAN verify is that the bridge preserves multi-character words
# when they appear in the Tencent response structure.
# =============================================================================


class TestClaim2_Tencent_WordSegmented:
    """Tencent ASR returns pre-segmented words (not per-character)."""

    def test_tencent_bridge_preserves_multichar_words(self) -> None:
        """Tencent ResultDetail words are preserved as-is by the Rust bridge.

        When Tencent returns Word='故事' (a multi-character word), the bridge
        should NOT split it into per-character tokens. This is the key difference
        from FunASR.

        We test this via the Python TencentRecognizer.monologues() which calls
        the Rust bridge internally.
        """
        from types import SimpleNamespace

        from batchalign.inference.languages.cantonese._tencent_api import TencentRecognizer

        rec = TencentRecognizer.__new__(TencentRecognizer)
        rec.lang_code = "yue"
        rec.provider_lang = "yue"

        # Simulate Tencent returning multi-character segmented words
        result_detail = [
            SimpleNamespace(
                StartMs=1000,
                SpeakerId=1,
                Words=[
                    # '故事' is ONE word in Tencent's segmentation
                    SimpleNamespace(Word="故事", OffsetStartMs=0, OffsetEndMs=400),
                    # '係' is a separate word
                    SimpleNamespace(Word="係", OffsetStartMs=450, OffsetEndMs=600),
                    # '好' is a separate word
                    SimpleNamespace(Word="好", OffsetStartMs=650, OffsetEndMs=800),
                ],
            ),
        ]
        payload = rec.monologues(result_detail)
        elements = payload["monologues"][0]["elements"]
        values = [el["value"] for el in elements]

        # Tencent's word boundaries should be preserved: 故事 is one token
        assert "故事" in values, (
            "Tencent's multi-character word '故事' should be preserved as one token, "
            "not split into '故' and '事'"
        )
        assert len(values) == 3, (
            "Should have 3 words (故事, 係, 好), not 4 characters"
        )


# =============================================================================
# Assumption 3: PyCantonese can do word segmentation for Cantonese
# =============================================================================
#
# PyCantonese's segment() function should group Cantonese characters into
# linguistically meaningful words. We test with known Cantonese words and phrases.
# =============================================================================


class TestClaim3_PyCantonese_Segmentation:
    """PyCantonese segment() produces linguistically meaningful word boundaries."""

    def test_segments_known_two_char_word(self) -> None:
        """'故事' (story) should be recognized as one word, not two characters.

        FunASR produces '故 事' as separate tokens, but '故事' is a single word.
        """
        import pycantonese

        result = pycantonese.segment("故事")
        assert result == ["故事"], (
            "'故事' is a single Cantonese word meaning 'story' — "
            "PyCantonese should keep it together"
        )

    def test_segments_sentence_into_words(self) -> None:
        """A Cantonese sentence should be segmented into words, not characters.

        '我想食嘢' = 我 (I) + 想 (want) + 食 (eat) + 嘢 (thing/stuff)
        These are all single-character words in Cantonese, so the segmenter
        should return 4 items — same as character count in this case.
        """
        import pycantonese

        result = pycantonese.segment("我想食嘢")
        # All characters preserved
        assert "".join(result) == "我想食嘢"
        # For this particular sentence, all words happen to be single characters
        # The important thing is that pycantonese.segment() runs without error
        # and produces valid output

    def test_segments_multichar_words_correctly(self) -> None:
        """Multi-character words should be grouped, not split.

        '佢哋好鍾意食嘢' = 佢哋 (they) + 好 (very) + 鍾意 (like) + 食 (eat) + 嘢 (stuff)
        Key: '佢哋' and '鍾意' are multi-character words that should NOT be split.
        """
        import pycantonese

        result = pycantonese.segment("佢哋好鍾意食嘢")
        joined = "".join(result)
        assert joined == "佢哋好鍾意食嘢", "All characters must be preserved"

        # '佢哋' should be one word, not split into '佢' + '哋'
        assert "佢哋" in result, (
            "'佢哋' (they) is a two-character word — "
            "PyCantonese should keep it together"
        )

        # '鍾意' should be one word, not split into '鍾' + '意'
        assert "鍾意" in result, (
            "'鍾意' (to like) is a two-character word — "
            "PyCantonese should keep it together"
        )

        # Total word count should be less than character count
        assert len(result) < len("佢哋好鍾意食嘢"), (
            f"Word count ({len(result)}) should be less than character count (7) — "
            "that's the whole point of word segmentation"
        )

    def test_our_segment_cantonese_wrapper_works(self) -> None:
        """Our _segment_cantonese() wrapper produces the same results.

        Verify that the batchalign3 wrapper function correctly delegates
        to PyCantonese and handles the per-character input format.
        """
        from batchalign.inference.morphosyntax import _segment_cantonese

        # Input: per-character tokens (as FunASR would produce)
        per_char_input = ["佢", "哋", "好", "鍾", "意", "食", "嘢"]

        result = _segment_cantonese(per_char_input)

        # Should produce fewer tokens than input (some chars grouped into words)
        assert len(result) < len(per_char_input), (
            f"Segmented output ({len(result)} words) should have fewer tokens "
            f"than per-char input ({len(per_char_input)} chars)"
        )
        # All characters preserved
        assert "".join(result) == "".join(per_char_input)
        # Multi-char words should appear
        assert "佢哋" in result, "'佢哋' should be grouped as one word"
        assert "鍾意" in result, "'鍾意' should be grouped as one word"


# =============================================================================
# Assumption 4: Stanza's Chinese tokenizer can segment Mandarin into words
# =============================================================================
#
# We cannot test Paraformer output directly (requires model download), but we
# CAN verify that Stanza's Chinese tokenizer (our --retokenize solution for
# Mandarin) is correctly configured and available.
# =============================================================================


class TestClaim4_Mandarin_Stanza_Segmentation:
    """Stanza's Chinese tokenizer can segment Mandarin into words.

    These tests load the real Stanza Chinese model and verify that
    ``tokenize_pretokenized=False`` produces word-level segmentation.
    Marked ``@golden`` because they download and load ML models.
    """

    @pytest.mark.golden
    def test_stanza_chinese_tokenizer_segments_multichar_words(self) -> None:
        """Stanza's zh tokenizer groups characters into multi-character words.

        '我去商店买东西' (I go to the store to buy things) — Stanza should
        produce fewer tokens than there are characters, proving it does
        word segmentation. Verified output: ['我', '去', '商店', '买', '东', '西']
        — '商店' (store) is correctly grouped; Stanza splits '东西' because
        the characters are individually meaningful ('east'/'west').
        """
        import stanza

        nlp = stanza.Pipeline(
            lang="zh",
            processors="tokenize",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=False,
        )

        doc = nlp("我去商店买东西")
        words = [word.text for sent in doc.sentences for word in sent.words]

        assert "".join(words) == "我去商店买东西", "All characters must be preserved"
        assert len(words) < 7, (
            f"Stanza should produce fewer than 7 tokens (got {len(words)}): {words} — "
            "at least some multi-character words should be grouped"
        )
        # '商店' (store) is a clear compound that Stanza groups
        assert "商店" in words, "'商店' (store) should be one word"

    @pytest.mark.golden
    def test_stanza_pretokenized_true_preserves_chars(self) -> None:
        """With pretokenized=True, Stanza does NOT re-segment — chars stay separate.

        This is the default (non-retokenize) behavior. Each input token is
        treated as a pre-segmented word.
        """
        import stanza

        nlp = stanza.Pipeline(
            lang="zh",
            processors="tokenize",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=True,
        )

        # Feed pre-tokenized single characters (space-separated = one per line)
        doc = nlp("我 去 商 店 买 东 西")
        words = [word.text for sent in doc.sentences for word in sent.words]

        assert len(words) == 7, (
            f"With pretokenized=True, all 7 characters should remain separate tokens "
            f"(got {len(words)}): {words}"
        )

    def test_language_code_mapping(self) -> None:
        """ISO 639-3 codes for Chinese map to Stanza's 'zh-hans' model."""
        from batchalign.worker._stanza_loading import iso3_to_alpha2

        assert iso3_to_alpha2("cmn") == "zh-hans"
        assert iso3_to_alpha2("zho") == "zh-hans"
        assert iso3_to_alpha2("yue") == "zh-hans"
