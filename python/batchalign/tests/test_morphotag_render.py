"""Hermetic regression tests for the UD → structured `%mor`/`%gra` analysis.

These exercise `batchalign.backends.morphosyntax.ud.render` directly with
hand-built fake Stanza `sentence` objects, so they run with no Stanza
install and no model download. The exact feature bundles below are the ones
Stanza emits for these sentences; the expected tier strings are what BA2
(`batchalign2`) produces, captured by running both engines side by side.

`render.parse_sentence` now returns a *structured* `SentenceAnalysis` (no tier
strings — those are built by the typed Rust writer). To assert byte-level
parity against BA2 we render the structure back to CHAT text **here, in the
test** with `_mor_str` / `_gra_str`, mirroring the typed writer's format. That
keeps the production path string-free while still pinning the exact output.

End-to-end parity (real Stanza vs BA2) is proven by the parity harness in
`scripts/parity/`; this file guards the porting logic itself against
regressions.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from types import SimpleNamespace

import pytest

from batchalign.backends.morphosyntax import stanza
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
    """Build an MWT-free sentence from (text, lemma, upos, feats, head, deprel)."""
    words = []
    tokens = []
    for i, (text, lemma, upos, feats, head, deprel) in enumerate(rows, start=1):
        words.append(FakeWord(text, lemma, upos, feats, head, deprel, id=i))
        tokens.append(FakeToken(text, [i]))
    return FakeSentence(words=words, tokens=tokens)


def _mor_str(analysis: render.SentenceAnalysis, delimiter: str) -> str:
    """Render the structured analysis back to a `%mor` tier body (test only),
    mirroring the typed `MorTier` writer: units `pos|lemma-feat...`, post-clitics
    joined with `~`, words space-joined, then the terminator."""
    words = []
    for word in analysis.words:
        units = []
        for u in word.units:
            s = f"{u.pos}|{u.lemma}"
            for feat in u.features:
                s += f"-{feat}"
            units.append(s)
        words.append("~".join(units))
    body = " ".join(words)
    if analysis.words:
        body += " " + delimiter
    return body


def _gra_str(analysis: render.SentenceAnalysis) -> str:
    """Render the structured analysis back to a `%gra` tier body (test only)."""
    rels = []
    for word in analysis.words:
        for u in word.units:
            rels.append(f"{u.index}|{u.head}|{u.deprel}")
    if analysis.terminator is not None:
        i, h, d = analysis.terminator
        rels.append(f"{i}|{h}|{d}")
    return " ".join(rels)


def test_english_declarative_matches_ba2():
    # "I want the red ball ."
    sent = _sentence(
        [
            (
                "I",
                "I",
                "PRON",
                "Case=Nom|Number=Sing|Person=1|PronType=Prs",
                2,
                "nsubj",
            ),
            (
                "want",
                "want",
                "VERB",
                "Mood=Ind|Number=Sing|Person=1|Tense=Pres|VerbForm=Fin",
                0,
                "root",
            ),
            ("the", "the", "DET", "Definite=Def|PronType=Art", 5, "det"),
            ("red", "red", "ADJ", "Degree=Pos", 5, "amod"),
            ("ball", "ball", "NOUN", "Number=Sing", 2, "obj"),
        ]
    )
    analysis = render.parse_sentence(sent, ".", [], "en")
    assert (
        _mor_str(analysis, ".")
        == "pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres-S1 det|the-Def-Art adj|red-S1 noun|ball-Acc ."
    )
    assert _gra_str(analysis) == "1|2|NSUBJ 2|0|ROOT 3|5|DET 4|5|AMOD 5|2|OBJ 6|2|PUNCT"


def test_code_switched_root_keeps_root_relation():
    sent = _sentence(
        [
            ("xbxxx", "xbxxx", "X", None, 0, "root"),
            ("friend", "friend", "NOUN", "Number=Sing", 1, "flat"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [["जी", "s"]], "en")

    assert _mor_str(analysis, ".") == "L2|xxx noun|friend ."
    assert _gra_str(analysis) == "1|0|ROOT 2|1|FLAT 3|1|PUNCT"


def test_code_switched_dependent_uses_flat_relation():
    sent = _sentence(
        [
            ("friend", "friend", "NOUN", "Number=Sing", 0, "root"),
            ("xbxxx", "xbxxx", "X", None, 1, "dep"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [["जी", "s"]], "en")

    assert _mor_str(analysis, ".") == "noun|friend L2|xxx ."
    assert _gra_str(analysis) == "1|0|ROOT 2|1|FLAT 3|1|PUNCT"


def test_english_aux_participle_matches_ba2():
    # "he is running very fast ."
    sent = _sentence(
        [
            (
                "he",
                "he",
                "PRON",
                "Case=Nom|Gender=Masc|Number=Sing|Person=3|PronType=Prs",
                3,
                "nsubj",
            ),
            (
                "is",
                "be",
                "AUX",
                "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin",
                3,
                "aux",
            ),
            ("running", "run", "VERB", "Tense=Pres|VerbForm=Part", 0, "root"),
            ("very", "very", "ADV", None, 5, "advmod"),
            ("fast", "fast", "ADV", "Degree=Pos", 3, "advmod"),
        ]
    )
    analysis = render.parse_sentence(sent, ".", [], "en")
    assert (
        _mor_str(analysis, ".")
        == "pron|he-Prs-Nom-S3 aux|be-Fin-Ind-Pres-S3 verb|run-Part-Pres-S adv|very adv|fast ."
    )
    assert (
        _gra_str(analysis)
        == "1|3|NSUBJ 2|3|AUX 3|0|ROOT 4|5|ADVMOD 5|3|ADVMOD 6|3|PUNCT"
    )


def test_mwt_contraction_groups_into_one_word():
    # "it's a big dog ." — the MWT "it's" → one word with a `~` post-clitic.
    words = [
        FakeWord(
            "it",
            "it",
            "PRON",
            "Case=Nom|Number=Sing|Person=3|PronType=Prs",
            5,
            "nsubj",
            id=1,
        ),
        FakeWord(
            "'s",
            "be",
            "AUX",
            "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin",
            5,
            "cop",
            id=2,
        ),
        FakeWord("a", "a", "DET", "Definite=Ind|PronType=Art", 5, "det", id=3),
        FakeWord("big", "big", "ADJ", "Degree=Pos", 5, "amod", id=4),
        FakeWord("dog", "dog", "NOUN", "Number=Sing", 0, "root", id=5),
    ]
    tokens = [
        FakeToken("it's", [1, 2]),  # MWT spanning words 1..2
        FakeToken("a", [3]),
        FakeToken("big", [4]),
        FakeToken("dog", [5]),
    ]
    sent = FakeSentence(words=words, tokens=tokens)
    analysis = render.parse_sentence(sent, ".", [], "en")
    # First word carries two units (it + 's) joined with `~`.
    assert len(analysis.words) == 4
    assert len(analysis.words[0].units) == 2
    assert (
        _mor_str(analysis, ".")
        == "pron|it-Prs-Nom-S3~aux|be-Fin-Ind-Pres-S3 det|a-Ind-Art adj|big-S1 noun|dog ."
    )
    assert _gra_str(analysis) == "1|5|NSUBJ 2|5|COP 3|5|DET 4|5|AMOD 5|0|ROOT 6|5|PUNCT"


def test_possessive_gerund_mwt_is_rescued_as_copula_progressive():
    # Stanza has emitted this analysis for "sink's overflowing": possessive
    # sink + PART 's + nominal gerund.  The structured rescue must update both
    # morphology and dependencies before MWT grouping.
    words = [
        FakeWord("sink", "sink", "NOUN", "Number=Sing", 3, "nmod:poss", id=1),
        FakeWord("'s", "'s", "PART", None, 1, "case", id=2),
        FakeWord("overflowing", "overflow", "NOUN", "Number=Sing", 0, "root", id=3),
    ]
    tokens = [FakeToken("sink's", [1, 2]), FakeToken("overflowing", [3])]

    analysis = render.parse_sentence(
        FakeSentence(words=words, tokens=tokens), ".", [], "en"
    )

    assert (
        _mor_str(analysis, ".")
        == "noun|sink~aux|be-Fin-Ind-Pres-S3 verb|overflow-Part-Pres-S ."
    )
    assert _gra_str(analysis) == "1|3|NSUBJ 2|3|AUX 3|0|ROOT 4|3|PUNCT"
    assert [anomaly.field for anomaly in analysis.anomalies] == [
        "english_copula_progressive"
    ]


def test_copula_progressive_rescue_does_not_rewrite_genuine_possessive():
    words = [
        FakeWord("boy", "boy", "NOUN", "Number=Sing", 3, "nmod:poss", id=1),
        FakeWord("'s", "'s", "PART", None, 1, "case", id=2),
        FakeWord("coat", "coat", "NOUN", "Number=Sing", 0, "root", id=3),
    ]
    tokens = [FakeToken("boy's", [1, 2]), FakeToken("coat", [3])]

    analysis = render.parse_sentence(
        FakeSentence(words=words, tokens=tokens), ".", [], "en"
    )

    assert "~part|s" in _mor_str(analysis, ".")
    assert analysis.anomalies == []


def test_italian_defect6_false_mwt_collapses_to_one_lexical_word():
    words = [
        FakeWord("picco", "picco", "VERB", None, 0, "root", id=1),
        FakeWord("lo", "il", "PRON", "Number=Sing|Person=3", 1, "obj", id=2),
        FakeWord("rosso", "rosso", "ADJ", "Gender=Masc|Number=Sing", 1, "amod", id=3),
    ]
    tokens = [FakeToken("piccolo", [1, 2]), FakeToken("rosso", [3])]

    analysis = render.parse_sentence(
        FakeSentence(words=words, tokens=tokens), ".", [], "it"
    )

    assert _mor_str(analysis, ".") == "adj|piccolo-S1 adj|rosso-S1 ."
    assert _gra_str(analysis) == "1|0|ROOT 2|1|AMOD 3|1|PUNCT"
    assert [anomaly.field for anomaly in analysis.anomalies] == ["italian_defect_6"]


def test_italian_genuine_compound_mwt_is_not_defect6_collapsed():
    words = [
        FakeWord("da", "dare", "VERB", "Mood=Imp|VerbForm=Fin", 0, "root", id=1),
        FakeWord("me", "me", "PRON", "Number=Sing|Person=1", 1, "iobj", id=2),
        FakeWord("la", "la", "PRON", "Gender=Fem|Number=Sing", 1, "obj", id=3),
    ]
    tokens = [FakeToken("dammela", [1, 2, 3])]

    analysis = render.parse_sentence(
        FakeSentence(words=words, tokens=tokens), ".", [], "it"
    )

    assert len(analysis.words[0].units) == 3
    assert not any(a.field == "italian_defect_6" for a in analysis.anomalies)


def test_italian_defect7_sentence_initial_la_collapses_to_article():
    words = [
        FakeWord(
            "il",
            "il",
            "DET",
            "Definite=Def|Gender=Masc|Number=Sing|PronType=Art",
            3,
            "det",
            id=1,
        ),
        FakeWord(
            "i",
            "il",
            "DET",
            "Definite=Def|Gender=Masc|Number=Plur|PronType=Art",
            1,
            "det",
            id=2,
        ),
        FakeWord(
            "storia",
            "storia",
            "NOUN",
            "Gender=Fem|Number=Sing",
            0,
            "root",
            id=3,
        ),
    ]
    tokens = [FakeToken("la", [1, 2]), FakeToken("storia", [3])]

    analysis = render.parse_sentence(
        FakeSentence(words=words, tokens=tokens), ".", [], "it"
    )

    assert _mor_str(analysis, ".") == "det|il-Fem-Def-Art-Sing noun|storia-Fem ."
    assert _gra_str(analysis) == "1|2|DET 2|0|ROOT 3|2|PUNCT"
    assert [anomaly.field for anomaly in analysis.anomalies] == ["italian_defect_7"]


def test_italian_defect9_dagliela_rewrites_only_mwt_head():
    words = [
        FakeWord("da", "da", "ADP", None, 0, "root", id=1),
        FakeWord(
            "glie",
            "gli",
            "PRON",
            "Gender=Masc|Number=Sing|Person=3|PronType=Prs",
            1,
            "iobj",
            id=2,
        ),
        FakeWord(
            "la",
            "la",
            "PRON",
            "Gender=Fem|Number=Sing|Person=3|PronType=Prs",
            1,
            "obj",
            id=3,
        ),
    ]
    tokens = [FakeToken("dagliela", [1, 2, 3])]

    analysis = render.parse_sentence(
        FakeSentence(words=words, tokens=tokens), ".", [], "it"
    )

    assert (
        _mor_str(analysis, ".")
        == "verb|dare-Fin-Imp-S2~pron|gli-Prs-S3~pron|la-Prs-S3 ."
    )
    assert _gra_str(analysis) == "1|0|ROOT 2|1|IOBJ 3|1|OBJ 4|1|PUNCT"
    assert len(analysis.words[0].units) == 3
    assert [anomaly.field for anomaly in analysis.anomalies] == ["italian_defect_9"]


def test_italian_defect10_posa_clitics_use_canonical_verb_lemma():
    for surface, clitic, gender in (
        ("posala", "la", "Fem"),
        ("posalo", "lo", "Masc"),
    ):
        words = [
            FakeWord(
                "posa",
                "posa",
                "VERB",
                "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
                0,
                "root",
                id=1,
            ),
            FakeWord(
                clitic,
                clitic,
                "PRON",
                f"Gender={gender}|Number=Sing|Person=3|PronType=Prs",
                1,
                "obj",
                id=2,
            ),
        ]
        analysis = render.parse_sentence(
            FakeSentence(words=words, tokens=[FakeToken(surface, [1, 2])]),
            ".",
            [],
            "it",
        )

        assert (
            _mor_str(analysis, ".")
            == f"verb|posare-Fin-Imp-S2~pron|{clitic}-Prs-S3 ."
        )
        assert _gra_str(analysis) == "1|0|ROOT 2|1|OBJ 3|1|PUNCT"
        assert [anomaly.field for anomaly in analysis.anomalies] == [
            "italian_defect_10"
        ]


def test_question_terminator_is_carried_through():
    # The terminator is applied downstream; %gra still ends with a PUNCT.
    sent = _sentence(
        [
            ("who", "who", "PRON", "PronType=Int", 0, "root"),
        ]
    )
    analysis = render.parse_sentence(sent, "?", [], "en")
    assert _mor_str(analysis, "?").endswith(" ?")
    assert analysis.terminator is not None
    assert analysis.terminator[2] == "PUNCT"


def test_rootless_ud_analysis_is_rejected():
    sent = _sentence(
        [
            ("the", "the", "DET", "Definite=Def|PronType=Art", 2, "det"),
            ("dog", "dog", "NOUN", "Number=Sing", 1, "nsubj"),
        ]
    )

    with pytest.raises(ValueError, match="exactly one root; found 0"):
        render.parse_sentence(sent, ".", [], "en")


def test_multiple_ud_roots_are_rejected():
    sent = _sentence(
        [
            ("hello", "hello", "INTJ", None, 0, "root"),
            ("world", "world", "NOUN", "Number=Sing", 0, "root"),
        ]
    )

    with pytest.raises(ValueError, match="exactly one root; found 2"):
        render.parse_sentence(sent, ".", [], "en")


def test_invalid_stanza_fields_preserve_surface_and_record_repairs():
    sent = _sentence(
        [
            ("hello", ".", None, None, 99, "<pad>"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [], "en")

    assert _mor_str(analysis, ".") == "x|hello ."
    assert analysis.words[0].units[0].deprel == "ROOT"
    repaired_fields = {anomaly.field for anomaly in analysis.anomalies}
    assert repaired_fields == {"lemma", "upos", "head", "deprel"}
    assert all(anomaly.text == "hello" for anomaly in analysis.anomalies)


def test_missing_lemma_preserves_surface_without_changing_valid_analysis():
    sent = _sentence(
        [
            ("world", None, "NOUN", "Number=Sing", 0, "root"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [], "en")

    assert _mor_str(analysis, ".") == "noun|world ."
    assert [anomaly.field for anomaly in analysis.anomalies] == ["lemma"]


def test_chinese_bogus_punctuation_lemma_preserves_han_surface():
    sent = _sentence(
        [
            ("苹果", "。", "NOUN", None, 0, "root"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [], "zh")

    assert _mor_str(analysis, ".") == "noun|苹果 ."
    assert analysis.words[0].units[0].deprel == "ROOT"
    assert [anomaly.field for anomaly in analysis.anomalies] == ["lemma"]
    assert analysis.anomalies[0].text == "苹果"


def test_stanza_repairs_are_logged_with_source_and_field(caplog):
    sent = _sentence(
        [
            ("hello", ".", None, None, 99, "<pad>"),
        ]
    )
    analysis = render.parse_sentence(sent, ".", [], "en")

    with caplog.at_level(logging.WARNING, logger="batchalign.stanza"):
        stanza._log_analysis_anomalies(
            SimpleNamespace(source_id="bad.cha", utterance_id=4), analysis
        )

    assert "source=bad.cha utterance=4" in caplog.text
    for field_name in ("lemma", "upos", "head", "deprel"):
        assert f"field={field_name}" in caplog.text


def test_italian_defect_8_expands_dammela_to_verb_and_clitics():
    sent = _sentence(
        [
            ("dammela", "dammelo", "ADJ", "Gender=Masc|Number=Sing", 0, "root"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [], "it")

    assert len(analysis.words) == 1
    assert _mor_str(analysis, ".") == (
        "verb|dare-Fin-Imp-S2~pron|me-Prs-S1~pron|la-Prs-S3 ."
    )
    assert _gra_str(analysis) == "1|0|ROOT 2|1|IOBJ 3|1|OBJ 4|1|PUNCT"
    assert [anomaly.field for anomaly in analysis.anomalies] == ["italian_defect_8"]


def test_italian_defects_12_and_13_restore_missing_clitic_expansions():
    cases = (
        ("aprilo", "aprire", "aprire", "lo", "italian_defect_12"),
        ("leggila", "leggilare", "leggere", "la", "italian_defect_13"),
    )

    for surface, observed_lemma, verb_lemma, pronoun, anomaly_field in cases:
        sent = _sentence([(surface, observed_lemma, "VERB", "VerbForm=Fin", 0, "root")])

        analysis = render.parse_sentence(sent, ".", [], "it")

        assert _mor_str(analysis, ".") == (
            f"verb|{verb_lemma}-Fin-Imp-S2~pron|{pronoun}-Prs-S3 ."
        )
        assert _gra_str(analysis) == "1|0|ROOT 2|1|OBJ 3|1|PUNCT"
        assert [anomaly.field for anomaly in analysis.anomalies] == [anomaly_field]


def test_italian_workaround_registry_is_numbered_and_retireable():
    from batchalign.backends.morphosyntax.ud.it.workarounds import (
        COMPOUND_IMPERATIVE_RULES,
    )

    assert {rule.defect for rule in COMPOUND_IMPERATIVE_RULES} == {8, 12, 13}
    assert len({rule.surface for rule in COMPOUND_IMPERATIVE_RULES}) == len(
        COMPOUND_IMPERATIVE_RULES
    )
    assert all("retire" in rule.retire_when for rule in COMPOUND_IMPERATIVE_RULES)


def test_italian_unknown_surface_is_not_rewritten():
    sent = _sentence(
        [
            ("bella", "bello", "ADJ", "Gender=Fem|Number=Sing", 0, "root"),
        ]
    )

    analysis = render.parse_sentence(sent, ".", [], "it")

    assert _mor_str(analysis, ".") == "adj|bello-S1 ."
    assert not analysis.anomalies
