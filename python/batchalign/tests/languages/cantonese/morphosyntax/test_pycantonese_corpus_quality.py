"""PyCantonese word segmentation quality on real child Cantonese corpus data.

Tests PyCantonese's segment() function against actual words from the CHILDES
CHCC Winston Cantonese corpus (child bilingual Cantonese-Mandarin speech).
This validates that our word segmentation solution works on the kind of data
that Angel Chan's team at PolyU will process.

Findings (2026-03-23):
- 2,280 unique pure-CJK words extracted from corpus
- 91% of multi-character words preserved as single tokens by PyCantonese
- The 9% that split are mostly multi-word phrases transcribed as single tokens
  (book titles, idiomatic expressions) — the splits are linguistically correct
- Key Cantonese words all handled correctly: 佢哋, 鍾意, 故事, 媽媽, 爸爸
"""

from __future__ import annotations

import os
import re
import os
from pathlib import Path

import pycantonese
import pytest


# Corpus path is resolved from an environment variable so this test can
# run locally against any developer's CHILDES checkout without hardcoding
# a machine-specific absolute path. Set `BATCHALIGN3_CHILDES_DATA_ROOT` to
# the directory containing the CHILDES data repositories (so the full
# path becomes `$BATCHALIGN3_CHILDES_DATA_ROOT/childes-other-data/...`).
# The test skips cleanly when the variable is unset or the directory is
# absent.
_DATA_ROOT_ENV = "BATCHALIGN3_CHILDES_DATA_ROOT"
_DATA_ROOT = os.environ.get(_DATA_ROOT_ENV)
CORPUS_DIR = (
    Path(_DATA_ROOT) / "childes-other-data" / "Biling" / "CHCC" / "Winston" / "Cantonese"
    if _DATA_ROOT
    else None
)

_SKIP_REASON = (
    "CHILDES CHCC Cantonese corpus not available locally "
    f"(set {_DATA_ROOT_ENV} to the CHILDES data root to enable)"
)

# Precompute the skip condition as a plain bool so type checkers do not
# need to narrow `CORPUS_DIR` through a boolean expression in the
# decorator argument.
_SKIP_CORPUS: bool = CORPUS_DIR is None or not CORPUS_DIR.exists()


def _extract_pure_cjk_words() -> set[str]:
    """Extract pure-CJK words from the CHILDES CHCC Winston Cantonese corpus."""
    assert CORPUS_DIR is not None, "guarded by skipif on CORPUS_DIR presence"
    words: set[str] = set()
    for fname in CORPUS_DIR.iterdir():
        if fname.suffix != ".cha":
            continue
        with open(fname) as f:
            for line in f:
                if not line.startswith("*") or "\t" not in line:
                    continue
                content = line.split("\t", 1)[1].strip()
                content = re.sub(r"\[.*?\]", "", content)
                content = re.sub(r"@\S+", "", content)
                content = re.sub(r"[.?!+/<>]", "", content)
                content = re.sub(r"&=\S+", "", content)
                content = re.sub(r"&\S+", "", content)
                content = re.sub(r"xxx|yyy|www", "", content)
                for w in content.split():
                    if all("\u4e00" <= c <= "\u9fff" for c in w) and w:
                        words.add(w)
    return words


@pytest.mark.skipif(_SKIP_CORPUS, reason=_SKIP_REASON)
class TestPyCantoneseCorpusQuality:
    """Validate PyCantonese segmentation quality on real child Cantonese data."""

    def test_multichar_preservation_rate_above_85_percent(self) -> None:
        """At least 85% of corpus multi-char words should be preserved as-is.

        91% was measured on 2026-03-23. We use 85% as the threshold to allow
        for minor dictionary changes in future PyCantonese versions.
        """
        words = _extract_pure_cjk_words()
        multichar = [w for w in words if len(w) > 1]
        assert len(multichar) > 100, f"Expected 100+ multi-char words, got {len(multichar)}"

        preserved = sum(
            1
            for w in multichar
            if pycantonese.segment(w) == [w]
        )
        rate = preserved / len(multichar)
        assert rate > 0.85, (
            f"PyCantonese preserved only {rate:.0%} of {len(multichar)} multi-char words "
            f"(expected >85%)"
        )

    def test_key_cantonese_words_preserved(self) -> None:
        """Common Cantonese words that users expect to see as single tokens."""
        must_preserve = [
            "佢哋",   # they
            "鍾意",   # like
            "故事",   # story
            "媽媽",   # mama
            "爸爸",   # papa
            "知道",   # know
            "多謝",   # thank you
            "聖誕",   # Christmas
            "飛機",   # airplane
            "蛋糕",   # cake
        ]
        for word in must_preserve:
            result = pycantonese.segment(word)
            assert result == [word], (
                f"'{word}' should be preserved as single word, got {result}"
            )

    def test_sentence_segmentation_reduces_char_count(self) -> None:
        """Realistic Cantonese sentences should produce fewer words than characters."""
        sentences = [
            ("佢哋好鍾意食嘢", 7, 5),     # they really like eating stuff
            ("媽媽買咗好多嘢", 7, 6),     # mama bought lots of stuff
            ("故事書好好睇", 6, 5),        # storybook is very good to read
        ]
        for text, max_chars, max_words in sentences:
            result = pycantonese.segment(text)
            assert len(result) <= max_words, (
                f"'{text}': expected ≤{max_words} words, got {len(result)}: {result}"
            )
            assert len(result) < max_chars, (
                f"'{text}': word count ({len(result)}) should be less than "
                f"char count ({max_chars})"
            )
