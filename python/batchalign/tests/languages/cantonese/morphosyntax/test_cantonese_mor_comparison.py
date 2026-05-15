"""Compare existing corpus %mor annotations against PyCantonese POS.

For each Cantonese corpus with existing %mor tiers, extract utterances
and compare the POS tags in the existing annotation against what
PyCantonese produces. This reveals:

1. Whether existing annotations use the same POS conventions as PyCantonese
2. Where our new pipeline would CHANGE existing annotations
3. Whether changes are improvements or regressions

Also checks @Comment headers for annotation provenance (CLAN, batchalign,
hand-annotated).

Corpora with existing %mor (see cantonese-corpus-inventory.md):
- LeeWongLeung: 243,466 utterances
- CHCC: 126,866
- EACMC: 98,660
- HKU CHILDES: 26,305
- GlobalTales: 19,849
- MAIN: 17,274
- WCT: 4,950
- Aphasia HKU: 876
"""

from __future__ import annotations

import os
import re
from pathlib import Path

import pycantonese
import pytest

# See test_cantonese_all_corpora.py for the `BATCHALIGN3_CHILDES_DATA_ROOT`
# convention. Tests in this file skip cleanly when the variable is unset.
_DATA_ROOT_ENV = "BATCHALIGN3_CHILDES_DATA_ROOT"
DATA_ROOT = Path(os.environ.get(_DATA_ROOT_ENV, "/nonexistent"))

CORPORA_WITH_MOR = {
    "LeeWongLeung": "childes-other-data/Chinese/Cantonese/LeeWongLeung",
    "HKU_CHILDES": "childes-other-data/Chinese/Cantonese/HKU",
    "MAIN": "childes-other-data/Chinese/Cantonese/MAIN",
    "GlobalTales": "childes-other-data/GlobalTales/Cantonese",
    "CHCC": "childes-other-data/Biling/CHCC/Winston/Cantonese",
    "EACMC": "childes-other-data/Biling/EACMC",
    "WCT": "ca-data/WCT",
    "Aphasia_HKU": "aphasia-data/Cantonese/Protocol/HKU",
}


def _extract_provenance(cha_path: Path) -> str:
    """Check @Comment headers for annotation tool provenance."""
    try:
        text = cha_path.read_text(errors="replace")
    except Exception:
        return "unknown"
    for line in text.splitlines():
        lower = line.lower()
        if line.startswith("@Comment:") and (
            "batchalign" in lower or "clan" in lower or "morpho" in lower
            or "annotat" in lower or "transcri" in lower
        ):
            return line.strip()
    return "no provenance comment"


def _extract_mor_pairs(
    corpus_path: Path, max_files: int = 5, max_utts: int = 20,
) -> list[dict]:
    """Extract (words, existing_pos, file, provenance) from CHAT files."""
    results = []
    cha_files = sorted(corpus_path.rglob("*.cha"))[:max_files]
    for f in cha_files:
        provenance = _extract_provenance(f)
        try:
            lines = f.read_text(errors="replace").splitlines()
        except Exception:
            continue
        i = 0
        collected = 0
        while i < len(lines) and collected < max_utts:
            if lines[i].startswith("*") and "\t" in lines[i]:
                main = lines[i].split("\t", 1)[1].strip()
                # Find %mor
                mor = None
                j = i + 1
                while j < len(lines) and not lines[j].startswith("*") and not lines[j].startswith("@"):
                    if lines[j].startswith("%mor:"):
                        mor = lines[j].split("\t", 1)[1].strip() if "\t" in lines[j] else lines[j][5:].strip()
                    j += 1
                if mor and any("\u4e00" <= c <= "\u9fff" for c in main):
                    # Parse existing %mor: extract POS|word pairs
                    existing_pos = {}
                    for token in mor.split():
                        if "|" in token:
                            pos, lemma = token.split("|", 1)
                            # Strip features after -
                            lemma_clean = lemma.split("-")[0].split("&")[0]
                            if any("\u4e00" <= c <= "\u9fff" for c in lemma_clean):
                                existing_pos[lemma_clean] = pos
                    if existing_pos:
                        # Get main tier CJK words
                        clean_main = re.sub(r"\[.*?\]", "", main)
                        clean_main = re.sub(r"@\S+", "", clean_main)
                        words = [w for w in clean_main.split()
                                 if any("\u4e00" <= c <= "\u9fff" for c in w)
                                 and all(c > "\u2e80" or c.isascii() for c in w)]
                        results.append({
                            "file": f.name,
                            "provenance": provenance,
                            "words": words,
                            "existing_pos": existing_pos,
                        })
                        collected += 1
                i = j if j > i else i + 1
            else:
                i += 1
    return results


@pytest.mark.parametrize("corpus_name,corpus_rel", list(CORPORA_WITH_MOR.items()))
class TestMorComparison:
    """Compare existing %mor against PyCantonese POS for each corpus."""

    def test_comparison(self, corpus_name: str, corpus_rel: str) -> None:
        corpus_path = DATA_ROOT / corpus_rel
        if not corpus_path.exists():
            pytest.skip(f"Corpus not available: {corpus_path}")

        pairs = _extract_mor_pairs(corpus_path)
        if not pairs:
            pytest.skip(f"No utterances with CJK %mor in {corpus_name}")

        # Check provenance
        provenances = set(p["provenance"] for p in pairs)
        print(f"\n  {corpus_name} provenance: {provenances}")

        # Compare POS
        agree = disagree = pyc_unknown = 0
        disagreements: list[tuple[str, str, str]] = []

        for pair in pairs:
            for lemma, existing_pos in pair["existing_pos"].items():
                pyc_tagged = dict(pycantonese.pos_tag([lemma]))
                pyc_pos = pyc_tagged.get(lemma, "X")

                if pyc_pos == "X":
                    pyc_unknown += 1
                    continue

                # Normalize existing POS to lowercase for comparison
                # Existing may use lowercase (verb, noun) or mixed
                existing_lower = existing_pos.lower()
                pyc_lower = pyc_pos.lower()

                if existing_lower == pyc_lower:
                    agree += 1
                else:
                    disagree += 1
                    if len(disagreements) < 10:
                        disagreements.append((lemma, existing_pos, pyc_pos))

        total = agree + disagree
        agree_rate = agree / total * 100 if total else 0

        print(
            f"  {corpus_name}: {total} comparable words, "
            f"{agree} agree ({agree_rate:.0f}%), {disagree} disagree, "
            f"{pyc_unknown} PyCantonese unknown"
        )
        if disagreements:
            print(f"  Sample disagreements:")
            for lemma, existing, pyc in disagreements[:5]:
                print(f"    {lemma}: existing={existing} pycantonese={pyc}")

        # We don't assert a specific agreement rate because existing
        # annotations may themselves be wrong (from batchalign2 with
        # Mandarin model). The point is to document the differences.
