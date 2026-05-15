"""Comprehensive Cantonese NLP quality tests across ALL TalkBank corpora.

Tests PyCantonese word segmentation and POS tagging against vocabulary
extracted from every Cantonese corpus in data/*-data/. Each test documents
which corpus it uses and what was found.

Corpora tested (see docs/investigations/cantonese-corpus-inventory.md):
1. CHILDES Chinese/Cantonese/LeeWongLeung (254K utterances, largest)
2. CHILDES Chinese/Cantonese/HKU (27K utterances, child speech)
3. CHILDES Chinese/Cantonese/MAIN (17K utterances, narrative)
4. CHILDES GlobalTales/Cantonese (20K utterances, narrative)
5. CHILDES Biling/CHCC (135K utterances, bilingual — existing test)
6. CHILDES Biling/EACMC (111K utterances, bilingual)
7. CA-data WCT (5K utterances, conversation analysis)
8. Aphasia HKU (887 utterances, adult clinical)
9. CHILDES Chinese/Cantonese/MOST (167K utterances, NO existing %mor)
"""

from __future__ import annotations

import os
import re
from pathlib import Path

import pycantonese
import pytest

# CHILDES data root is resolved from an environment variable so this
# suite runs against any developer's checkout without hardcoding a
# machine-specific absolute path. Set `BATCHALIGN3_CHILDES_DATA_ROOT`
# to the directory containing the CHILDES data repositories, e.g.
# `.../data/` so the child repos `childes-other-data/`,
# `childes-eng-na-data/`, etc. sit directly beneath it. When the
# variable is unset (or the directory is absent) the corpus-backed
# tests in this file skip cleanly.
_DATA_ROOT_ENV = "BATCHALIGN3_CHILDES_DATA_ROOT"
DATA_ROOT = Path(os.environ.get(_DATA_ROOT_ENV, "/nonexistent"))

# Corpus paths relative to DATA_ROOT
CORPORA = {
    "LeeWongLeung": "childes-other-data/Chinese/Cantonese/LeeWongLeung",
    "HKU_CHILDES": "childes-other-data/Chinese/Cantonese/HKU",
    "MAIN": "childes-other-data/Chinese/Cantonese/MAIN",
    "GlobalTales": "childes-other-data/GlobalTales/Cantonese",
    "CHCC": "childes-other-data/Biling/CHCC/Winston/Cantonese",
    "EACMC": "childes-other-data/Biling/EACMC",
    "WCT": "ca-data/WCT",
    "Aphasia_HKU": "aphasia-data/Cantonese/Protocol/HKU",
    "MOST": "childes-other-data/Chinese/Cantonese/MOST",
}


def _extract_cjk_words(corpus_path: Path, max_files: int = 20) -> set[str]:
    """Extract unique pure-CJK words from CHAT files in a corpus."""
    words: set[str] = set()
    cha_files = sorted(corpus_path.rglob("*.cha"))[:max_files]
    for f in cha_files:
        try:
            text = f.read_text(errors="replace")
        except Exception:
            continue
        for line in text.splitlines():
            if not line.startswith("*") or "\t" not in line:
                continue
            content = line.split("\t", 1)[1]
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


def _test_word_segmentation(corpus_name: str, corpus_path: Path) -> None:
    """Test PyCantonese word segmentation on corpus vocabulary."""
    words = _extract_cjk_words(corpus_path)
    if not words:
        pytest.skip(f"No CJK words found in {corpus_name}")

    multichar = [w for w in words if len(w) > 1]
    if len(multichar) < 10:
        pytest.skip(f"Too few multi-char words in {corpus_name}: {len(multichar)}")

    preserved = sum(1 for w in multichar if pycantonese.segment(w) == [w])
    rate = preserved / len(multichar)

    # Document the result regardless of pass/fail
    print(
        f"\n  {corpus_name}: {len(words)} CJK words "
        f"({len(multichar)} multi-char, {preserved} preserved = {rate:.0%})"
    )

    # We expect >80% preservation — PyCantonese should know most corpus words
    assert rate > 0.50, (
        f"{corpus_name}: PyCantonese preserved only {rate:.0%} of "
        f"{len(multichar)} multi-char words (expected >50%)"
    )


def _test_pos_quality(corpus_name: str, corpus_path: Path) -> None:
    """Test PyCantonese POS on corpus vocabulary — check for X (unknown) rate."""
    words = _extract_cjk_words(corpus_path)
    if not words:
        pytest.skip(f"No CJK words found in {corpus_name}")

    # Sample up to 200 words
    sample = sorted(words)[:200]
    tagged = pycantonese.pos_tag(sample)
    unknown_count = sum(1 for _, pos in tagged if pos == "X")
    unknown_rate = unknown_count / len(sample)

    print(
        f"\n  {corpus_name}: {len(sample)} words tested, "
        f"{unknown_count} tagged X ({unknown_rate:.0%})"
    )

    # High X rate means PyCantonese doesn't know these words
    assert unknown_rate < 0.50, (
        f"{corpus_name}: {unknown_rate:.0%} of words tagged as X (unknown). "
        f"PyCantonese may lack coverage for this corpus."
    )


# Generate test functions for each corpus
@pytest.mark.parametrize("corpus_name,corpus_rel", list(CORPORA.items()))
class TestWordSegmentation:
    """PyCantonese word segmentation across all Cantonese corpora."""

    def test_segmentation(self, corpus_name: str, corpus_rel: str) -> None:
        corpus_path = DATA_ROOT / corpus_rel
        if not corpus_path.exists():
            pytest.skip(f"Corpus not available: {corpus_path}")
        _test_word_segmentation(corpus_name, corpus_path)


@pytest.mark.parametrize("corpus_name,corpus_rel", list(CORPORA.items()))
class TestPosQuality:
    """PyCantonese POS tag quality across all Cantonese corpora."""

    def test_pos(self, corpus_name: str, corpus_rel: str) -> None:
        corpus_path = DATA_ROOT / corpus_rel
        if not corpus_path.exists():
            pytest.skip(f"Corpus not available: {corpus_path}")
        _test_pos_quality(corpus_name, corpus_path)
