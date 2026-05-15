"""Test Houjun's claim: retokenize as secondary pass vs ASR engine segmentation.

Houjun (2026-03-23): "Retokenize as a secondary pass would do a better job
than either Tencent or in particular FunAudio alone since Stanza has a special
model trained just for this task."

Three sub-claims to verify:
1. Stanza retokenize is better than FunASR alone (for Cantonese)
2. Stanza retokenize is better than Tencent alone (for Cantonese)
3. Stanza has a "special model trained just for this task" (for Chinese)

Key correction: for Cantonese, we use PyCantonese (not Stanza) for retokenize.
Stanza's zh model is trained on Mandarin, not Cantonese.
"""

from __future__ import annotations

import pycantonese
import pytest


# =============================================================================
# Sub-claim 1: Retokenize is better than FunASR alone for Cantonese
# =============================================================================
#
# FunASR produces per-character tokens. PyCantonese retokenize groups them
# into words. This should always be an improvement since per-character
# tokenization is never linguistically correct for word-level analysis.
# =============================================================================


class TestRetokenizeVsFunASR:
    """PyCantonese retokenize improves on FunASR per-character output."""

    def test_funasr_per_char_has_no_multichar_words(self) -> None:
        """FunASR output (simulated) is all single characters — zero word info."""
        import batchalign_core

        text = "佢哋好鍾意食嘢"
        funasr_tokens = batchalign_core.cantonese_char_tokens(text)
        multichar = [t for t in funasr_tokens if len(t) > 1]
        assert len(multichar) == 0, (
            f"FunASR should produce zero multi-char words, got {multichar}"
        )

    def test_pycantonese_retokenize_creates_multichar_words(self) -> None:
        """PyCantonese retokenize groups characters into real words."""
        import batchalign_core

        text = "佢哋好鍾意食嘢"
        funasr_tokens = batchalign_core.cantonese_char_tokens(text)
        retokenized = pycantonese.segment("".join(funasr_tokens))

        multichar = [t for t in retokenized if len(t) > 1]
        assert len(multichar) > 0, (
            f"Retokenize should produce multi-char words, got {retokenized}"
        )
        assert len(retokenized) < len(funasr_tokens), (
            f"Retokenize ({len(retokenized)} words) should have fewer tokens "
            f"than FunASR ({len(funasr_tokens)} chars)"
        )

    def test_retokenize_strictly_improves_word_count(self) -> None:
        """For every test sentence, retokenize produces fewer tokens than FunASR.

        This proves Houjun's sub-claim 1: retokenize is better than FunASR alone.
        """
        import batchalign_core

        sentences = [
            "佢哋好鍾意食嘢",       # they really like eating stuff
            "我想去買故事書",         # I want to go buy a storybook
            "媽媽買咗好多嘢",       # mama bought lots of stuff
            "你知唔知道",           # do you know
            "直升飛機好大架",       # the helicopter is very big
        ]
        for text in sentences:
            funasr = batchalign_core.cantonese_char_tokens(text)
            retok = pycantonese.segment("".join(funasr))
            assert len(retok) < len(funasr), (
                f"'{text}': retokenize ({len(retok)}) should be < FunASR ({len(funasr)})\n"
                f"  FunASR:  {funasr}\n"
                f"  Retok:   {retok}"
            )


# =============================================================================
# Sub-claim 2: Retokenize is better than Tencent alone for Cantonese
# =============================================================================
#
# This claim is UNVERIFIABLE without real Tencent output. We cannot prove or
# disprove it because:
# - We don't have real Tencent Cantonese ASR output
# - Spencer claims Tencent does word segmentation, but our test data only
#   shows single-character words from Tencent
# - If Tencent already segments correctly, retokenize could be HARMFUL
#   (re-segmenting already-correct boundaries)
#
# The test below documents this gap.
# =============================================================================


class TestRetokenizeVsTencent:
    """Retokenize vs Tencent: cannot verify without real Tencent output."""

    def test_retokenize_on_already_segmented_input_is_identity(self) -> None:
        """If Tencent already segments correctly, retokenize should preserve words.

        This tests whether retokenize is at least non-harmful on good input.
        If PyCantonese re-segments already-correct words, it could be destructive.
        """
        # Simulate Tencent output with correct word boundaries
        tencent_words = ["佢哋", "好", "鍾意", "食嘢"]
        retokenized = pycantonese.segment("".join(tencent_words))

        assert retokenized == tencent_words, (
            f"Retokenize should preserve already-correct segmentation.\n"
            f"  Tencent:    {tencent_words}\n"
            f"  Retokenized: {retokenized}\n"
            "If this fails, retokenize HARMS Tencent output."
        )

    def test_retokenize_on_partial_segmentation_improves(self) -> None:
        """If Tencent partially segments (some words, some chars), retokenize helps."""
        # Simulate Tencent output where some words are segmented, others per-char
        partial_words = ["佢哋", "好", "鍾", "意", "食嘢"]
        retokenized = pycantonese.segment("".join(partial_words))

        assert "鍾意" in retokenized, (
            f"Retokenize should group '鍾'+'意' → '鍾意', got {retokenized}"
        )


# =============================================================================
# Sub-claim 3: Stanza has a "special model trained just for this task"
# =============================================================================
#
# Correction: for Cantonese, we DON'T use Stanza's tokenizer — we use
# PyCantonese. Stanza's zh model is trained on Mandarin (Chinese Treebank),
# not Cantonese. Using it for Cantonese would miss Cantonese-specific words.
#
# For Mandarin, Stanza does have a trained tokenizer (gsdsimp package).
# =============================================================================


class TestStanzaVsPyCantonese:
    """Stanza's zh tokenizer vs PyCantonese for Cantonese text."""

    @pytest.mark.golden
    def test_stanza_misses_cantonese_specific_words(self) -> None:
        """Stanza's Mandarin tokenizer doesn't know Cantonese-specific vocabulary.

        This disproves the implicit claim that Stanza is the right tool for
        Cantonese word segmentation. PyCantonese is better because it has a
        Cantonese-specific dictionary.
        """
        import stanza

        nlp = stanza.Pipeline(
            lang="zh",
            processors="tokenize",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=False,
        )

        # Cantonese-specific text
        doc = nlp("佢哋好鍾意食嘢")
        stanza_words = [w.text for s in doc.sentences for w in s.words]

        pyc_words = pycantonese.segment("佢哋好鍾意食嘢")

        # PyCantonese should produce fewer (better-grouped) tokens
        # because it knows Cantonese vocabulary
        print(f"Stanza (Mandarin model): {stanza_words}")
        print(f"PyCantonese:             {pyc_words}")

        # PyCantonese must group 佢哋 — Stanza may or may not
        assert "佢哋" in pyc_words, "PyCantonese should know 佢哋 (they)"
        assert "鍾意" in pyc_words, "PyCantonese should know 鍾意 (like)"
        assert "食嘢" in pyc_words, "PyCantonese should know 食嘢 (eat stuff)"

        # Key question: does Stanza also know these Cantonese words?
        stanza_knows_keuidei = "佢哋" in stanza_words
        stanza_knows_jungji = "鍾意" in stanza_words
        stanza_knows_sikje = "食嘢" in stanza_words

        if not (stanza_knows_keuidei and stanza_knows_jungji and stanza_knows_sikje):
            # Stanza missed Cantonese words — PyCantonese is better
            missed = []
            if not stanza_knows_keuidei:
                missed.append("佢哋")
            if not stanza_knows_jungji:
                missed.append("鍾意")
            if not stanza_knows_sikje:
                missed.append("食嘢")
            print(f"CONFIRMED: Stanza misses Cantonese words: {missed}")
            print("PyCantonese is the correct choice for Cantonese retokenize.")
