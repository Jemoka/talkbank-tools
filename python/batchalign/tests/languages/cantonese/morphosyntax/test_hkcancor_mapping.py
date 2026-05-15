"""HKCanCor POS tag mapping to Universal Dependencies — validation tests.

Validates that PyCantonese's HKCanCor corpus maps cleanly to UD tags
and assesses its viability as Stanza training data augmentation.

These tests use real PyCantonese data (no mocks, no Stanza models).
"""

from __future__ import annotations

from collections import Counter

import pycantonese
from pycantonese.pos_tagging.tagger import hkcancor_to_ud


def _load_corpus() -> pycantonese.corpus.CHAT:
    """Load HKCanCor corpus (cached by PyCantonese)."""
    return pycantonese.hkcancor()


def test_hkcancor_corpus_size() -> None:
    """HKCanCor has the expected corpus size."""
    corpus = _load_corpus()
    tokens = corpus.tokens()
    utts = corpus.utterances()

    assert len(tokens) > 150_000, f"Expected >150K tokens, got {len(tokens)}"
    assert len(utts) > 16_000, f"Expected >16K utterances, got {len(utts)}"


def test_hkcancor_ud_mapping_covers_99_percent() -> None:
    """At least 99% of non-punctuation tokens map to a UD tag (not X)."""
    corpus = _load_corpus()
    tokens = corpus.tokens()

    non_punct = [t for t in tokens if t.pos and t.pos.strip()]
    mapped_x = sum(1 for t in non_punct if hkcancor_to_ud(t.pos) == "X")

    x_rate = mapped_x / len(non_punct)
    assert x_rate < 0.01, (
        f"X-mapped rate {x_rate:.3f} ({mapped_x}/{len(non_punct)}) exceeds 1%"
    )


def test_hkcancor_has_no_dependency_annotations() -> None:
    """HKCanCor has zero dependency (GRA) annotations — cannot train depparse."""
    corpus = _load_corpus()
    tokens = corpus.tokens()

    has_gra = sum(1 for t in tokens if t.gra is not None)
    assert has_gra == 0, f"Expected 0 tokens with GRA, got {has_gra}"


def test_hkcancor_jyutping_coverage() -> None:
    """Majority of tokens have jyutping romanization."""
    corpus = _load_corpus()
    tokens = corpus.tokens()

    has_jp = sum(1 for t in tokens if t.jyutping is not None and t.jyutping)
    coverage = has_jp / len(tokens)
    assert coverage > 0.75, f"Jyutping coverage {coverage:.2f} below 75%"


def test_hkcancor_ud_distribution_has_all_major_tags() -> None:
    """Mapped corpus has all major UD tags (VERB, NOUN, ADJ, ADV, PRON, etc.)."""
    corpus = _load_corpus()
    tokens = corpus.tokens()

    ud_counter: Counter[str] = Counter()
    for t in tokens:
        if t.pos and t.pos.strip():
            ud_counter[hkcancor_to_ud(t.pos)] += 1

    required = {"VERB", "NOUN", "ADJ", "ADV", "PRON", "AUX", "ADP", "PART", "INTJ", "NUM"}
    missing = required - set(ud_counter.keys())
    assert not missing, f"Missing UD tags: {missing}"

    # VERB should be the most common substantive tag
    assert ud_counter["VERB"] > 20_000, (
        f"Expected >20K VERB tokens, got {ud_counter['VERB']}"
    )


def test_hkcancor_mapping_table_completeness() -> None:
    """The mapping table covers all POS tags actually seen in the corpus."""
    corpus = _load_corpus()
    tokens = corpus.tokens()

    corpus_tags = {t.pos for t in tokens if t.pos and t.pos.strip()}
    mapping = hkcancor_to_ud()
    mapped_tags = set(mapping.keys())

    # Tags in corpus but not in mapping → would map to X
    unmapped = corpus_tags - mapped_tags
    # Allow a small number of edge cases
    assert len(unmapped) < 5, (
        f"Too many unmapped tags ({len(unmapped)}): {unmapped}"
    )


def test_hkcancor_classifier_maps_to_noun() -> None:
    """Cantonese classifiers (量詞, tag 'q') map to NOUN in UD.

    This is the most debatable mapping — UD has no CLASSIFIER tag.
    Document the decision: classifiers are nominal in UD convention.
    """
    assert hkcancor_to_ud("q") == "NOUN"

    # Verify classifiers are substantial in the corpus
    corpus = _load_corpus()
    tokens = corpus.tokens()
    classifiers = [t for t in tokens if t.pos == "q"]
    assert len(classifiers) > 4000, (
        f"Expected >4K classifier tokens, got {len(classifiers)}"
    )


def test_hkcancor_sentence_final_particles_map_to_part() -> None:
    """Sentence-final particles (語氣助詞, tags 'y', 'y1') map to PART.

    This is important for Cantonese — SFPs are extremely common and
    distinguishing them from other parts of speech matters for syntax.
    """
    assert hkcancor_to_ud("y") == "PART"
    assert hkcancor_to_ud("y1") == "PART"

    corpus = _load_corpus()
    tokens = corpus.tokens()
    sfps = [t for t in tokens if t.pos in ("y", "y1")]
    assert len(sfps) > 17_000, (
        f"Expected >17K SFP tokens, got {len(sfps)}"
    )
