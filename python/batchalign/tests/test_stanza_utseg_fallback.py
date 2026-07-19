"""Deterministic tests for the opt-in Stanza UtSeg fallback."""

from __future__ import annotations

from types import SimpleNamespace

import pytest
import typer

from batchalign.backends.utseg.stanza_fallback import _compute_assignments
from batchalign.cli.transcribe import _build_utseg
from batchalign.lang import LanguageCode


class _Tree:
    def __init__(self, label: str, children: list["_Tree"] | None = None) -> None:
        self.label = label
        self.children = children or []

    def is_leaf(self) -> bool:
        return not self.children


def _leaf(word: str) -> _Tree:
    return _Tree(word)


def test_coordinated_constituency_clauses_form_two_utterances():
    tree = _Tree(
        "ROOT",
        [
            _Tree("S", [_leaf("I"), _leaf("like"), _leaf("tea")]),
            _Tree("CC", [_leaf("and")]),
            _Tree("S", [_leaf("you"), _leaf("like"), _leaf("coffee")]),
        ],
    )
    nlp = lambda _text: SimpleNamespace(  # noqa: E731
        sentences=[SimpleNamespace(constituency=tree)]
    )

    assert _compute_assignments(
        ["I", "like", "tea", "and", "you", "like", "coffee"], nlp
    ) == [0, 0, 0, 1, 1, 1, 1]


def test_unsupported_language_refuses_silent_unsegmented_output():
    class Backends:
        StanzaUtSegBackend = staticmethod(lambda **kwargs: ("stanza", kwargs))

    spanish = LanguageCode.from_str("spa")
    with pytest.raises(typer.BadParameter, match="--utseg-fallback-stanza"):
        _build_utseg(Backends, spanish)
    assert _build_utseg(Backends, spanish, stanza_fallback=True) == (
        "stanza",
        {"lang": "spa"},
    )
