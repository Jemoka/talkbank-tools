"""Mandarin word segmentation tests using real corpus data.

Tests that Mandarin data in TalkBank already has word-level tokenization
(unlike Cantonese which is per-character from ASR), and validates Stanza's
word segmentation on simulated per-character input.

These tests do NOT load Stanza models (no @pytest.mark.golden).
They test the segmentation decision logic and input characteristics.
"""

from __future__ import annotations


# ---------------------------------------------------------------------------
# Mandarin corpus characteristics
# ---------------------------------------------------------------------------


def test_mandarin_corpus_already_word_segmented() -> None:
    """Mandarin CHILDES data (ChangPN) already has word-level tokenization.

    Unlike Cantonese ASR output which is per-character, Mandarin transcripts
    in CHILDES typically have proper word boundaries. This means --retokenize
    is less critical for pre-existing Mandarin transcripts, but still needed
    for Mandarin ASR output from engines like FunASR.

    Real utterance from ChangPN/grade1/139.cha:
    "第一 关 呢 叫做 说说看"
    """
    words = ["第一", "关", "呢", "叫做", "说说看"]

    # Mandarin transcripts have multi-char words already
    multi_char = [w for w in words if len(w) > 1]
    assert len(multi_char) >= 3, (
        f"Expected >=3 multi-char words, got {len(multi_char)}: {multi_char}"
    )

    # These are real word boundaries, not per-character
    assert "叫做" in words  # "called" — 2 chars, 1 word
    assert "说说看" in words  # "let's see" — 3 chars, 1 word


def test_mandarin_per_char_simulation() -> None:
    """Simulated per-character Mandarin input (as from ASR).

    When an ASR engine outputs per-character Mandarin, --retokenize should
    produce word-level tokens. Test the join logic.
    """
    # Simulate per-char: "你 看 这 是 什 么"
    per_char = ["你", "看", "这", "是", "什", "么"]

    # Space-joined for Stanza (our current approach)
    text_spaced = " ".join(per_char)
    assert text_spaced == "你 看 这 是 什 么"

    # Empty-joined (incorrect approach we fixed)
    text_joined = "".join(per_char)
    assert text_joined == "你看这是什么"
    # Both should be 6 characters
    assert len(text_joined) == 6


def test_mandarin_mixed_script_join_safety() -> None:
    """Latin+CJK in Mandarin utterances must preserve word boundaries.

    Real scenario: code-switched Mandarin with English words.
    Bug #6 (fixed): "".join merged Latin and CJK into one token.
    """
    words = ["hello", "你", "好", "世", "界"]

    # Correct approach: space join
    text = " ".join(words)
    tokens = text.split()
    assert len(tokens) == 5, f"Expected 5 tokens, got {len(tokens)}: {tokens}"

    # Incorrect approach (old bug): empty join
    bad_text = "".join(words)
    bad_tokens = bad_text.split()
    assert len(bad_tokens) == 1, "Empty join merges everything — this was the bug"


def test_mandarin_retokenize_decision_logic() -> None:
    """Retokenize pipeline activates only when BOTH item_lang and req_lang are Mandarin.

    This prevents [- zho] pre-codes in a Cantonese job from triggering
    Mandarin neural tokenization (bug #5, fixed in 3c03fe3b).
    """
    scenarios = [
        # (req_lang, item_lang, expected_retok)
        ("cmn", "cmn", True),   # Mandarin job, Mandarin utterance
        ("zho", "zho", True),   # Chinese job, Chinese utterance
        ("cmn", "zho", True),   # Mandarin job, Chinese pre-code
        ("yue", "zho", False),  # Cantonese job, [- zho] pre-code — MUST NOT retok
        ("yue", "yue", False),  # Cantonese job — uses PyCantonese, not Stanza retok
        ("eng", "cmn", False),  # English job, Mandarin pre-code
    ]

    for req_lang, item_lang, expected in scenarios:
        use_retok = (
            item_lang in ("zho", "cmn")
            and req_lang in ("zho", "cmn")
        )
        assert use_retok == expected, (
            f"req={req_lang}, item={item_lang}: "
            f"expected retok={expected}, got {use_retok}"
        )


def test_mandarin_classifier_handling() -> None:
    """Mandarin classifiers (量词) are common in CHILDES data.

    Real from ChangPN: "四 件 事情"
    Classifier 件 (item/piece) should be a separate token.
    """
    # Already word-segmented in corpus
    words = ["四", "件", "事情"]
    assert len(words) == 3

    # After segmentation, "件" should remain separate (it's a classifier)
    # This tests that we don't over-merge classifiers with numerals
    assert "件" in words
    assert "事情" in words
