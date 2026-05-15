"""Verification: Tencent ASR word segmentation for Cantonese.

Spencer Cheng (PolyU) reported that Tencent ASR does word segmentation.
This test documents the empirical finding from running real Cantonese
audio through Tencent ASR on net (2026-03-23).

Finding: Tencent produces 100% single-character output for Cantonese.
25 CJK words, 0 multi-character. Spencer's claim is DISPROVED for
Cantonese. All Cantonese ASR engines produce per-character output.

Provenance:
- Audio: yue_hku_clip.mp3 (A023.mp4 lines 11-16, 26s aphasia speech)
- Engine: Tencent Cloud ASR via batchalign3 on net
- Credentials: ~/.batchalign.ini on net
- Script: scripts/check-media/verify_tencent_cantonese.sh

Raw Tencent output (single utterance):
*PAR: 話 就 會 走 神 又 熟 豬 公 土 一 日 喺 度 這 很 多 好 有 啲 人 睇 到 最 早 .
(25 single-character words, 0 multi-character words)
"""

from __future__ import annotations


def test_tencent_cantonese_output_is_per_character() -> None:
    """Document that Tencent Cantonese ASR output is per-character.

    This is NOT a live API test — it records the empirically verified result.
    The actual Tencent API call was run via:
        bash scripts/check-media/verify_tencent_cantonese.sh

    If Tencent changes their model to include word segmentation, this test
    should be updated with fresh evidence from a new API call.
    """
    # Raw Tencent output for yue_hku_clip.mp3 (2026-03-23)
    tencent_output = "話 就 會 走 神 又 熟 豬 公 土 一 日 喺 度 這 很 多 好 有 啲 人 睇 到 最 早"
    words = tencent_output.split()

    cjk_words = [w for w in words if any("\u4e00" <= c <= "\u9fff" for c in w)]
    multi_char = [w for w in cjk_words if len(w) > 1]

    assert len(cjk_words) == 25, f"Expected 25 CJK words, got {len(cjk_words)}"
    assert len(multi_char) == 0, (
        f"Tencent Cantonese output should have 0 multi-char words "
        f"(per-character only). Got {len(multi_char)}: {multi_char}. "
        "If >0, Tencent may have improved their word segmentation."
    )
