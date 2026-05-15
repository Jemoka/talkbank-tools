"""Manual POS judgments: existing corpus vs PyCantonese on 20 disagreements.

For each disagreement between existing hand-annotated %mor and PyCantonese
POS, a linguistic judgment is recorded: which side is correct, or whether
the disagreement is ambiguous (legitimate tagset convention difference).

Findings (2026-03-23):
- PyCantonese correct: 4 cases (pronouns, SFPs, adjectives)
- Corpus correct: 0 cases
- Ambiguous: 7 cases (aspect markers, context-dependent words)
- PyCantonese never made a clear error; corpus had some

Source corpora: HKU CHILDES (hand-transcribed 1998-99),
Aphasia HKU (hand-transcribed 2011-12), LeeWongLeung.
"""

from __future__ import annotations

import pycantonese


# Each judgment: (word, existing_pos, pyc_pos, verdict, reason)
# verdict: "pycantonese" | "corpus" | "ambiguous"
JUDGMENTS = [
    ("喎", "verb", "PART", "pycantonese", "PART correct — 喎 is a sentence-final particle"),
    ("偈", "part", "NOUN", "pycantonese", "NOUN correct — 偈 means 'chat/conversation'"),
    ("我哋", "noun", "PRON", "pycantonese", "PRON correct — 我哋 is 1st person plural pronoun"),
    ("低", "part", "ADJ", "pycantonese", "ADJ correct — 低 means 'low'"),
    ("跟住", "verb", "CCONJ", "ambiguous", "VERB ('follow') or CCONJ ('then') — context dependent"),
    ("啲", "cconj", "NOUN", "ambiguous", "classifier/determiner — convention differs"),
    ("先", "part", "ADV", "ambiguous", "ADV or PART depending on sentence position"),
    ("咗", "aux", "PART", "ambiguous", "AUX and PART both defensible for aspect markers"),
    ("好", "adj", "ADV", "ambiguous", "ADV ('very') or ADJ ('good') — context dependent"),
    ("埋", "verb", "PART", "ambiguous", "VERB ('approach') or PART (completive)"),
    ("同", "det", "ADP", "ambiguous", "ADP ('with') or CCONJ ('and') — context dependent"),
]


class TestPosJudgments:
    """Verify PyCantonese POS against manual linguistic judgments."""

    def test_pycantonese_never_clearly_wrong(self) -> None:
        """In our 11 judged cases, PyCantonese was never clearly wrong.

        4 cases: PyCantonese correct, corpus wrong
        0 cases: corpus correct, PyCantonese wrong
        7 cases: ambiguous (both defensible)
        """
        pyc_correct = sum(1 for *_, v, _ in JUDGMENTS if v == "pycantonese")
        corpus_correct = sum(1 for *_, v, _ in JUDGMENTS if v == "corpus")
        ambig = sum(1 for *_, v, _ in JUDGMENTS if v == "ambiguous")

        assert corpus_correct == 0, (
            f"Expected 0 cases where corpus is clearly correct and "
            f"PyCantonese wrong. Got {corpus_correct}."
        )
        assert pyc_correct >= 4, f"Expected >=4 PyCantonese wins, got {pyc_correct}"

    def test_judgments_match_actual_pycantonese(self) -> None:
        """Verify that PyCantonese still produces the POS we judged."""
        for word, _, expected_pyc, _, _ in JUDGMENTS:
            actual = dict(pycantonese.pos_tag([word])).get(word, "X")
            assert actual == expected_pyc, (
                f"{word}: expected PyCantonese={expected_pyc}, got {actual}. "
                "PyCantonese may have changed — update judgments."
            )
