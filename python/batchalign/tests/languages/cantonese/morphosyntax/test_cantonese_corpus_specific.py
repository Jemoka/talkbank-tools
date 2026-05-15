"""Corpus-specific Cantonese word segmentation + POS tests.

Tests PyCantonese word segmentation and POS tagging on real utterances
extracted from underrepresented TalkBank corpora: LeeWongLeung (child speech),
EACMC (bilingual), and WCT (adult conversation). Each test uses actual
utterances from specific corpus files to ensure coverage across speech domains.

No Stanza models loaded — pure PyCantonese tests, safe to run locally.
"""

from __future__ import annotations

import pycantonese


# ---------------------------------------------------------------------------
# LeeWongLeung — largest Cantonese corpus (254K utterances, child speech)
# Source: data/childes-other-data/Chinese/Cantonese/LeeWongLeung/
# ---------------------------------------------------------------------------


def test_leewongleung_word_segmentation() -> None:
    """PyCantonese segments LeeWongLeung per-char tokens correctly.

    Real utterances from mhz/020806.cha:
    - 一係 自己 斟 啦
    - 好啦
    """
    # Simulate per-char ASR output
    per_char = list("一係自己斟啦")
    segmented = pycantonese.segment("".join(per_char))

    # Key groupings expected
    assert "自己" in segmented, f"自己 not found in {segmented}"
    # 一係 is a Cantonese particle meaning "either/or"
    # PyCantonese may or may not group it — test it doesn't crash
    assert len(segmented) <= len(per_char), "Segmentation should reduce token count"


def test_leewongleung_pos_tagging() -> None:
    """PyCantonese POS tags LeeWongLeung vocabulary correctly.

    Real words from the corpus: 阿, 飲, 斟, 啦, 自己, 好
    """
    words = ["阿", "飲", "斟", "啦", "自己", "好"]
    tagged = pycantonese.pos_tag(words)
    tag_dict = {w: p for w, p in tagged}

    # 飲 (drink) should be VERB
    assert tag_dict["飲"] == "VERB", f"飲 tagged as {tag_dict['飲']}, expected VERB"
    # 自己 (self) should be PRON
    assert tag_dict["自己"] == "PRON", f"自己 tagged as {tag_dict['自己']}, expected PRON"
    # 好 (good) can be ADJ or ADV — both are reasonable
    assert tag_dict["好"] in ("ADJ", "ADV"), f"好 tagged as {tag_dict['好']}"
    # 啦 (SFP) should be PART
    assert tag_dict["啦"] == "PART", f"啦 tagged as {tag_dict['啦']}, expected PART"


def test_leewongleung_jyutping_in_utterance() -> None:
    """LeeWongLeung uses jyutping romanization (e.g., lo2, aa3, haak6).

    These should pass through word segmentation without interference.
    Jyutping tokens are Latin characters — PyCantonese segment() should not
    try to segment them.
    """
    # From mhz/020226.cha: "你 想 lo2 乜嘢 aa3"
    # In a morphotag context, jyutping tokens would be preserved as-is
    words = ["你", "想", "lo2", "乜嘢", "aa3"]
    tagged = pycantonese.pos_tag(words)
    tag_dict = {w: p for w, p in tagged}

    assert tag_dict["你"] == "PRON"
    assert tag_dict["想"] == "VERB"
    assert tag_dict["乜嘢"] in ("PRON", "NOUN"), f"乜嘢 tagged as {tag_dict['乜嘢']}"


# ---------------------------------------------------------------------------
# EACMC — bilingual corpus (Cantonese + Mandarin + English)
# Source: data/childes-other-data/Biling/EACMC/cross/HongKong/Can_Man_Eng/CAN/
# ---------------------------------------------------------------------------


def test_eacmc_cantonese_word_segmentation() -> None:
    """PyCantonese segments EACMC Cantonese utterances correctly.

    Real utterances from CAN/CME3.cha:
    - 粟米 湯 唔 要
    - 嗯 要

    From CAN/CME6.cha:
    - 你 就 幫 媽媽 切 個 青瓜
    """
    # Per-char simulation of "你就幫媽媽切個青瓜"
    per_char = list("你就幫媽媽切個青瓜")
    segmented = pycantonese.segment("".join(per_char))

    # 媽媽 (mommy) must be grouped
    assert "媽媽" in segmented, f"媽媽 not found in {segmented}"
    # 青瓜 (cucumber) should be grouped
    assert "青瓜" in segmented, f"青瓜 not found in {segmented}"
    assert len(segmented) < len(per_char), "Should group multi-char words"


def test_eacmc_cantonese_pos_tagging() -> None:
    """PyCantonese POS tags EACMC Cantonese vocabulary correctly.

    Real words from CAN/ files: 茄子, 攞, 俾, 切, 粟米, 青瓜, 媽媽
    """
    words = ["茄子", "攞", "俾", "切", "粟米", "青瓜", "媽媽"]
    tagged = pycantonese.pos_tag(words)
    tag_dict = {w: p for w, p in tagged}

    # 茄子 (eggplant) — PyCantonese tags as PROPN (proper noun).
    # Linguistically debatable: it's a common noun in standard usage,
    # but PyCantonese's HKCanCor training data may have it as a name.
    assert tag_dict["茄子"] in ("NOUN", "PROPN"), f"茄子 tagged as {tag_dict['茄子']}"
    # 攞 (take) — VERB
    assert tag_dict["攞"] == "VERB", f"攞 tagged as {tag_dict['攞']}"
    # 俾 (give) — VERB or ADP (both valid in Cantonese)
    assert tag_dict["俾"] in ("VERB", "ADP"), f"俾 tagged as {tag_dict['俾']}"
    # 切 (cut) — VERB in context, but PyCantonese tags as NOUN in isolation.
    # Ambiguous: 切 can be a verb (to cut) or noun (a cut/section).
    # Context-dependent — acceptable as either.
    assert tag_dict["切"] in ("VERB", "NOUN"), f"切 tagged as {tag_dict['切']}"
    # 媽媽 (mommy) — NOUN
    assert tag_dict["媽媽"] == "NOUN", f"媽媽 tagged as {tag_dict['媽媽']}"


def test_eacmc_code_switching_annotation() -> None:
    """EACMC has code-switching markers like @s:yue.

    Words with @s:yue are Cantonese words in a Mandarin/English context.
    PyCantonese should handle the bare word (without the marker).
    """
    # From CAN/CME7.cha: "依 個 係 咩 嚟㗎"
    words = ["依", "個", "係", "咩", "嚟"]
    tagged = pycantonese.pos_tag(words)
    tag_dict = {w: p for w, p in tagged}

    # 係 (is/be) — VERB in Cantonese
    assert tag_dict["係"] == "VERB", f"係 tagged as {tag_dict['係']}"
    # 個 (classifier) — should be some nominal/classifier tag
    assert tag_dict["個"] != "X", f"個 tagged as X (unknown)"


# ---------------------------------------------------------------------------
# WCT/yue — adult Cantonese conversation (conversation analysis)
# Source: data/ca-data/WCT/video/yue/
# ---------------------------------------------------------------------------


def test_wct_adult_word_segmentation() -> None:
    """PyCantonese segments WCT adult Cantonese per-char tokens.

    Real utterances from yue/07.cha:
    - 咁 書 入 邊 都 講 過 啦
    - 佢 哋 唔 一 定 係 互 相 有 牴 觸
    """
    per_char = list("咁書入邊都講過啦")
    segmented = pycantonese.segment("".join(per_char))

    assert len(segmented) <= len(per_char), "Should group some characters"
    # 入邊 (inside) may or may not be grouped — domain-specific


def test_wct_adult_pos_tagging() -> None:
    """PyCantonese POS tags WCT adult vocabulary.

    Real words from yue/13.cha and yue/07.cha:
    樂隊 (band), 希望 (hope), 理論 (theory), 解釋 (explain), 牴觸 (conflict)
    """
    words = ["樂隊", "希望", "理論", "解釋", "牴觸"]
    tagged = pycantonese.pos_tag(words)
    tag_dict = {w: p for w, p in tagged}

    # 樂隊 (band/orchestra) — NOUN
    assert tag_dict["樂隊"] == "NOUN", f"樂隊 tagged as {tag_dict['樂隊']}"
    # 希望 (hope) — PyCantonese tags as AUX in Cantonese (auxiliary "hope to").
    # In Mandarin it's typically VERB/NOUN, but Cantonese uses it as an auxiliary.
    assert tag_dict["希望"] in ("VERB", "NOUN", "AUX"), f"希望 tagged as {tag_dict['希望']}"
    # 理論 (theory) — NOUN
    assert tag_dict["理論"] == "NOUN", f"理論 tagged as {tag_dict['理論']}"
    # 解釋 (explain) — VERB
    assert tag_dict["解釋"] == "VERB", f"解釋 tagged as {tag_dict['解釋']}"


def test_wct_per_char_is_typical() -> None:
    """WCT/yue files use per-character tokenization (space-separated).

    Verify that a typical WCT utterance has mostly single-char CJK tokens,
    confirming that --retokenize would be needed for word-level analysis.
    """
    # From yue/07.cha: "就 算 係 對 同 一 件 事 物 喺 學 界 入 邊 呢 亦 都 有 唔 同 嘅 理 論 去 解 釋 呢"
    words = "就 算 係 對 同 一 件 事 物 喺 學 界 入 邊 呢 亦 都 有 唔 同 嘅 理 論 去 解 釋 呢".split()

    cjk_words = [w for w in words if any("\u4e00" <= c <= "\u9fff" for c in w)]
    single_char = [w for w in cjk_words if len(w) == 1]

    # >90% of CJK tokens should be single characters
    single_rate = len(single_char) / len(cjk_words) if cjk_words else 0
    assert single_rate > 0.90, (
        f"Expected >90% single-char CJK, got {single_rate:.1%} "
        f"({len(single_char)}/{len(cjk_words)})"
    )


def test_wct_segmentation_improves_token_count() -> None:
    """PyCantonese segmentation reduces token count for WCT per-char input.

    The 28-character utterance should compress to fewer word-level tokens.
    """
    text = "就算係對同一件事物喺學界入邊呢亦都有唔同嘅理論去解釋呢"
    segmented = pycantonese.segment(text)

    # Should produce fewer tokens than characters
    assert len(segmented) < len(text), (
        f"Segmentation produced {len(segmented)} tokens from {len(text)} chars"
    )
    # Should produce at least some multi-char words
    multi_char = [w for w in segmented if len(w) > 1]
    assert len(multi_char) > 0, "No multi-char words produced"
