"""Hermetic regression tests for the UD → CHAT `%mor`/`%gra` renderer.

These exercise `batchalign.backends.morphosyntax.ud.render` directly with
hand-built fake Stanza `sentence` objects, so they run with no Stanza
install and no model download. The exact feature bundles below are the ones
Stanza emits for these sentences; the expected tier strings are what BA2
(`batchalign2`) produces, captured by running both engines side by side.

End-to-end parity (real Stanza vs BA2) is proven by the parity harness in
`scripts/parity/`; this file guards the porting logic itself against
regressions.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from batchalign.backends.morphosyntax.ud import render


@dataclass
class FakeWord:
    text: str
    lemma: str
    upos: str
    feats: str | None
    head: int
    deprel: str
    id: int = 0


@dataclass
class FakeToken:
    text: str
    id: list  # 1-based id list; len>1 marks an MWT


@dataclass
class FakeSentence:
    words: list = field(default_factory=list)
    tokens: list = field(default_factory=list)


def _sentence(rows: list[tuple]) -> FakeSentence:
    """Build a single-MWT-free sentence from (text, lemma, upos, feats, head, deprel)."""
    words = []
    tokens = []
    for i, (text, lemma, upos, feats, head, deprel) in enumerate(rows, start=1):
        words.append(FakeWord(text, lemma, upos, feats, head, deprel, id=i))
        tokens.append(FakeToken(text, [i]))
    return FakeSentence(words=words, tokens=tokens)


def test_english_declarative_matches_ba2():
    # "I want the red ball ."
    sent = _sentence([
        ("I", "I", "PRON", "Case=Nom|Number=Sing|Person=1|PronType=Prs", 2, "nsubj"),
        ("want", "want", "VERB", "Mood=Ind|Number=Sing|Person=1|Tense=Pres|VerbForm=Fin", 0, "root"),
        ("the", "the", "DET", "Definite=Def|PronType=Art", 5, "det"),
        ("red", "red", "ADJ", "Degree=Pos", 5, "amod"),
        ("ball", "ball", "NOUN", "Number=Sing", 2, "obj"),
    ])
    mor, gra = render.parse_sentence(sent, ".", [], "en")
    assert mor == "pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres-S1 det|the-Def-Art adj|red-S1 noun|ball-Acc ."
    assert gra == "1|2|NSUBJ 2|5|ROOT 3|5|DET 4|5|AMOD 5|2|OBJ 6|2|PUNCT"


def test_english_aux_participle_matches_ba2():
    # "he is running very fast ."
    sent = _sentence([
        ("he", "he", "PRON", "Case=Nom|Gender=Masc|Number=Sing|Person=3|PronType=Prs", 3, "nsubj"),
        ("is", "be", "AUX", "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin", 3, "aux"),
        ("running", "run", "VERB", "Tense=Pres|VerbForm=Part", 0, "root"),
        ("very", "very", "ADV", None, 5, "advmod"),
        ("fast", "fast", "ADV", "Degree=Pos", 3, "advmod"),
    ])
    mor, gra = render.parse_sentence(sent, ".", [], "en")
    assert mor == "pron|he-Prs-Nom-S3 aux|be-Fin-Ind-Pres-S3 verb|run-Part-Pres-S adv|very adv|fast ."
    assert gra == "1|3|NSUBJ 2|3|AUX 3|5|ROOT 4|5|ADVMOD 5|3|ADVMOD 6|3|PUNCT"


def test_question_terminator_is_carried_through():
    # The terminator the runner recovers is appended verbatim to %mor.
    sent = _sentence([
        ("who", "who", "PRON", "PronType=Int", 0, "root"),
    ])
    mor, _ = render.parse_sentence(sent, "?", [], "en")
    assert mor.endswith(" ?")
