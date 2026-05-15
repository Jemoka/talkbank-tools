"""Tagset analysis: existing corpus %mor vs PyCantonese POS conventions.

Analyzes the disagreement patterns between existing corpus annotations
and PyCantonese POS tags to distinguish:
1. True tagset equivalences (same meaning, different label)
2. Genuine POS disagreements (different linguistic analysis)
3. Systematic biases in either system

Raw agreement is ~49%. After normalizing known equivalences
(aux↔part, sconj↔cconj, propn↔noun), agreement rises to ~51%.
The remaining ~49% disagreement is genuine — the existing annotations
and PyCantonese assign fundamentally different categories to many
Cantonese words.

This is NOT evidence that PyCantonese is wrong. The existing annotations
were produced by CLAN MOR or batchalign2 with a Mandarin model, using
different POS conventions and making systematic errors on Cantonese
vocabulary.

Provenance (from @Comment headers):
- MAIN: "Batchalign 0.7.23, ASR Engine funaudio"
- GlobalTales: "Batchalign 0.7.17, ASR Engine tencent"
- HKU CHILDES: hand-transcribed 1998-99 (pre-UD tagset)
- Aphasia HKU: hand-transcribed 2011-12 (pre-UD tagset)
"""

from __future__ import annotations

import re
import os
from collections import Counter
from pathlib import Path

import pycantonese
import pytest

# See test_cantonese_all_corpora.py for the `BATCHALIGN3_CHILDES_DATA_ROOT`
# convention. Tests in this file skip cleanly when the variable is unset.
_DATA_ROOT_ENV = "BATCHALIGN3_CHILDES_DATA_ROOT"
DATA_ROOT = Path(os.environ.get(_DATA_ROOT_ENV, "/nonexistent"))

# Skip entire module when corpus data is not available (CI, fresh clones).
pytestmark = pytest.mark.skipif(
    not DATA_ROOT.exists(),
    reason=f"Corpus data not found at {DATA_ROOT}",
)

# Known POS equivalences where both labels are linguistically defensible
EQUIVALENCES = {
    # Aspect markers (咗, 緊, 過) can be AUX or PART
    frozenset({"aux", "part"}): "aspect markers",
    # Subordinating vs coordinating conjunction is a gradient
    frozenset({"sconj", "cconj"}): "conjunction subtype",
    # Proper vs common noun boundary is fuzzy for names used as common nouns
    frozenset({"propn", "noun"}): "proper/common noun boundary",
}

CORPORA = {
    "LeeWongLeung": "childes-other-data/Chinese/Cantonese/LeeWongLeung",
    "HKU_CHILDES": "childes-other-data/Chinese/Cantonese/HKU",
    "MAIN": "childes-other-data/Chinese/Cantonese/MAIN",
    "CHCC": "childes-other-data/Biling/CHCC/Winston/Cantonese",
    "Aphasia_HKU": "aphasia-data/Cantonese/Protocol/HKU",
}


def _collect_pairs(max_files_per_corpus: int = 5) -> Counter:
    """Collect (existing_pos, pyc_pos) pairs across all corpora."""
    pairs: Counter = Counter()
    for _name, rel in CORPORA.items():
        path = DATA_ROOT / rel
        if not path.exists():
            continue
        for f in sorted(path.rglob("*.cha"))[:max_files_per_corpus]:
            try:
                lines = f.read_text(errors="replace").splitlines()
            except Exception:
                continue
            i = 0
            while i < len(lines):
                if lines[i].startswith("*") and "\t" in lines[i]:
                    mor = None
                    j = i + 1
                    while j < len(lines) and not lines[j].startswith("*") and not lines[j].startswith("@"):
                        if lines[j].startswith("%mor:"):
                            mor = (
                                lines[j].split("\t", 1)[1].strip()
                                if "\t" in lines[j]
                                else lines[j][5:].strip()
                            )
                        j += 1
                    if mor:
                        for token in mor.split():
                            if "|" in token:
                                pos, lemma = token.split("|", 1)
                                lemma_clean = lemma.split("-")[0].split("&")[0]
                                if any("\u4e00" <= c <= "\u9fff" for c in lemma_clean):
                                    pyc = dict(pycantonese.pos_tag([lemma_clean]))
                                    pyc_pos = pyc.get(lemma_clean, "X")
                                    if pyc_pos != "X":
                                        pairs[(pos.lower(), pyc_pos.lower())] += 1
                    i = j if j > i else i + 1
                else:
                    i += 1
    return pairs


class TestTagsetAnalysis:
    """Document POS tagset disagreement patterns."""

    @pytest.fixture(scope="class")
    def pairs(self) -> Counter:
        return _collect_pairs()

    def test_raw_agreement_documented(self, pairs: Counter) -> None:
        """Raw agreement rate between existing %mor and PyCantonese POS."""
        total = sum(pairs.values())
        agree = sum(count for (ex, pyc), count in pairs.items() if ex == pyc)
        rate = agree / total if total else 0

        print(f"\n  Raw agreement: {agree}/{total} ({rate:.0%})")
        # Document but don't assert a specific rate — this is observational
        assert total > 1000, f"Too few pairs collected: {total}"

    def test_normalized_agreement(self, pairs: Counter) -> None:
        """Agreement after normalizing known equivalences."""
        total = sum(pairs.values())
        agree = 0
        for (ex, pyc), count in pairs.items():
            if ex == pyc:
                agree += count
            elif frozenset({ex, pyc}) in EQUIVALENCES:
                agree += count

        rate = agree / total if total else 0
        equiv_count = agree - sum(
            c for (ex, pyc), c in pairs.items() if ex == pyc
        )
        print(f"\n  Normalized agreement: {agree}/{total} ({rate:.0%})")
        print(f"  Equivalences normalized: {equiv_count}")

    def test_top_disagreement_patterns(self, pairs: Counter) -> None:
        """Identify the largest disagreement patterns.

        These reveal where existing annotations and PyCantonese have
        fundamentally different POS conventions.
        """
        disagreements = {
            (ex, pyc): count
            for (ex, pyc), count in pairs.items()
            if ex != pyc and frozenset({ex, pyc}) not in EQUIVALENCES
        }
        top = sorted(disagreements.items(), key=lambda x: -x[1])[:10]

        print(f"\n  Top genuine disagreements:")
        for (ex, pyc), count in top:
            print(f"    existing={ex:>6} → pycantonese={pyc:<6} ({count}x)")

    def test_equivalence_counts(self, pairs: Counter) -> None:
        """Count how many disagreements are just tagset equivalences."""
        for equiv_set, reason in EQUIVALENCES.items():
            tags = list(equiv_set)
            count = sum(
                c
                for (ex, pyc), c in pairs.items()
                if ex in equiv_set and pyc in equiv_set and ex != pyc
            )
            print(f"\n  Equivalence {tags}: {count} ({reason})")
