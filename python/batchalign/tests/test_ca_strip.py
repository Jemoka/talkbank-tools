"""Tests for the CA-notation regex used in Stanza pre-tokenization.

Hermetic — exercises only the regex; no stanza/torch imports required.
"""

from __future__ import annotations

from batchalign.backends.morphosyntax.stanza import _CA_NOTATION_RE


def test_strips_pitch_arrows() -> None:
    assert _CA_NOTATION_RE.sub("", "hello ↑there↓ friend") == "hello there friend"


def test_strips_circle_dot() -> None:
    assert _CA_NOTATION_RE.sub("", "°quietly° spoken") == "quietly spoken"


def test_strips_pitch_accents() -> None:
    assert _CA_NOTATION_RE.sub("", "H* word L* end") == " word  end"


def test_strips_overlap_brackets() -> None:
    assert _CA_NOTATION_RE.sub("", "he ⌈said⌉ ⌊yes⌋") == "he said yes"


def test_preserves_normal_punctuation() -> None:
    s = "hello, world!  what about it?"
    assert _CA_NOTATION_RE.sub("", s) == s


def test_no_op_on_plain_text() -> None:
    s = "the quick brown fox jumps over the lazy dog"
    assert _CA_NOTATION_RE.sub("", s) == s
