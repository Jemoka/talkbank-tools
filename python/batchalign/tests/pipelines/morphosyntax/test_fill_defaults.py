"""Tests for UdWord Pydantic validation of Stanza output.

Stanza's doc.to_dict() can omit required fields in two cases:
1. MWT Range tokens (id=[start, end]) — only have id and text
2. Regular tokens where a processor (e.g. lemma) fails silently

The Rust UdWord struct requires: id, text, lemma, upos, head, deprel.
Missing any of these causes serde deserialization failure.  The Pydantic
``UdWord`` model mirrors the Rust struct and fills safe defaults.
"""

from __future__ import annotations
from typing import Any

from batchalign.inference.morphosyntax import (
    UdWord,
    validate_ud_words as _validate_ud_words,
)


# --- Direct model tests ---


def test_udword_complete_token() -> None:
    """Complete token should pass validation unchanged."""
    w = UdWord.model_validate({
        "id": 1, "text": "hello", "lemma": "hello",
        "upos": "INTJ", "head": 0, "deprel": "root",
        "xpos": "UH", "feats": None,
    })
    assert w.lemma == "hello"
    assert w.upos == "INTJ"
    assert w.head == 0
    assert w.deprel == "root"
    assert w.xpos == "UH"


def test_udword_missing_lemma_defaults_to_text() -> None:
    """Regular token missing lemma should default to surface text."""
    w = UdWord.model_validate({
        "id": 14, "text": "assistante",
        "upos": "NOUN", "head": 6, "deprel": "conj",
    })
    assert w.lemma == "assistante"


def test_udword_range_token_gets_empty_lemma() -> None:
    """MWT Range token should get empty string lemma (not surface text)."""
    w = UdWord.model_validate({"id": [2, 3], "text": "au"})
    assert w.lemma == ""
    assert w.upos == "X"
    assert w.head == 0
    assert w.deprel == "dep"


def test_udword_missing_upos() -> None:
    """Missing upos should default to 'X'."""
    w = UdWord.model_validate({
        "id": 1, "text": "foo", "lemma": "foo",
        "head": 0, "deprel": "root",
    })
    assert w.upos == "X"


def test_udword_missing_head() -> None:
    """Missing head should default to 0."""
    w = UdWord.model_validate({
        "id": 1, "text": "foo", "lemma": "foo",
        "upos": "NOUN", "deprel": "root",
    })
    assert w.head == 0


def test_udword_missing_deprel() -> None:
    """Missing deprel should default to 'dep'."""
    w = UdWord.model_validate({
        "id": 1, "text": "foo", "lemma": "foo",
        "upos": "NOUN", "head": 0,
    })
    assert w.deprel == "dep"


def test_udword_pad_deprel_sanitized() -> None:
    """Stanza <PAD> deprel should be replaced with safe default 'dep'."""
    w = UdWord.model_validate({
        "id": 3, "text": "etxean", "lemma": "etxe",
        "upos": "NOUN", "head": 0, "deprel": "<PAD>",
    })
    assert w.deprel == "dep"


def test_udword_angle_bracket_deprel_sanitized() -> None:
    """Any angle-bracketed deprel (e.g. <UNK>) should be sanitized."""
    w = UdWord.model_validate({
        "id": 1, "text": "foo", "lemma": "foo",
        "upos": "NOUN", "head": 0, "deprel": "<UNK>",
    })
    assert w.deprel == "dep"


def test_udword_normal_deprel_preserved() -> None:
    """Normal deprel values must not be affected by PAD sanitization."""
    w = UdWord.model_validate({
        "id": 1, "text": "hello", "lemma": "hello",
        "upos": "INTJ", "head": 0, "deprel": "root",
    })
    assert w.deprel == "root"


def test_udword_extra_fields_preserved() -> None:
    """Extra fields (ner, start_char, etc.) should be kept."""
    w = UdWord.model_validate({
        "id": 1, "text": "Paris", "lemma": "Paris",
        "upos": "PROPN", "head": 0, "deprel": "root",
        "ner": "B-LOC", "start_char": 0, "end_char": 5,
    })
    d = w.model_dump()
    assert d["ner"] == "B-LOC"
    assert d["start_char"] == 0


def test_udword_tuple_id_coerced_to_list() -> None:
    """Stanza uses tuples for Range IDs; Pydantic should accept them."""
    w = UdWord.model_validate({"id": (2, 3), "text": "au"})
    assert w.id == [2, 3]
    assert w.lemma == ""


# --- Integration: _validate_ud_words ---


def test_validate_ud_words_fills_all_sentences() -> None:
    """All sentences and all tokens should be validated."""
    sents: list[list[dict[str, Any]]] = [
        [
            {"id": 1, "text": "je", "upos": "PRON", "head": 2, "deprel": "nsubj"},
            {"id": [2, 3], "text": "au"},
            {"id": 2, "text": "à", "upos": "ADP", "head": 4, "deprel": "case"},
            {"id": 3, "text": "le", "upos": "DET", "head": 4, "deprel": "det"},
        ],
        [
            {"id": 1, "text": "oui", "head": 0, "deprel": "root"},
        ],
    ]
    _validate_ud_words(sents)

    # Sentence 1: regular token missing lemma
    assert sents[0][0]["lemma"] == "je"
    # Sentence 1: Range token
    assert sents[0][1]["lemma"] == ""
    assert sents[0][1]["upos"] == "X"
    # Sentence 1: component tokens missing lemma
    assert sents[0][2]["lemma"] == "à"
    assert sents[0][3]["lemma"] == "le"
    # Sentence 2: missing lemma and upos
    assert sents[1][0]["lemma"] == "oui"
    assert sents[1][0]["upos"] == "X"
