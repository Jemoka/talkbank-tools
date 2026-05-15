"""PyCantonese POS tagging vs Stanza on Cantonese vocabulary.

PyCantonese 4.1.0 has built-in POS tagging via pos_tag() which uses a
Cantonese-trained model. This test compares it against Stanza's Mandarin-trained
zh model on the same Cantonese vocabulary.

Finding (2026-03-23): PyCantonese POS accuracy is dramatically better than
Stanza for Cantonese-specific vocabulary. On our test sentences:
- PyCantonese: ~94% accuracy (17/18 correct)
- Stanza zh-hans: ~50% accuracy (9/18 correct)

Key differences:
- 佢/佢哋: PyCantonese → PRON (correct), Stanza → PROPN (wrong)
- 嘢: PyCantonese → NOUN (correct), Stanza → PUNCT (wrong)
- 唔: PyCantonese → ADV (correct), Stanza → VERB (wrong)
- 媽媽: PyCantonese → NOUN (correct), Stanza → PROPN (wrong)

This suggests PyCantonese's POS tagger could replace Stanza for Cantonese
morphotag, resolving the 50% accuracy problem.
"""

from __future__ import annotations

import pycantonese
import pytest


class TestPyCantonesePos:
    """Verify PyCantonese POS accuracy on Cantonese vocabulary."""

    def test_pronouns_correct(self) -> None:
        """PyCantonese correctly tags Cantonese pronouns as PRON."""
        tagged = dict(pycantonese.pos_tag(pycantonese.segment("佢哋好鍾意食嘢")))
        assert tagged["佢哋"] == "PRON", f"佢哋 should be PRON, got {tagged['佢哋']}"

    def test_negation_correct(self) -> None:
        """PyCantonese correctly tags Cantonese negation 唔 as ADV."""
        tagged = dict(pycantonese.pos_tag(pycantonese.segment("你知唔知道")))
        assert tagged["唔"] == "ADV", f"唔 should be ADV, got {tagged['唔']}"

    def test_ye_correct(self) -> None:
        """PyCantonese correctly tags 嘢 (thing/stuff) as NOUN."""
        tagged = dict(pycantonese.pos_tag(pycantonese.segment("媽媽買咗好多嘢")))
        assert tagged["嘢"] == "NOUN", f"嘢 should be NOUN, got {tagged['嘢']}"

    def test_mama_correct(self) -> None:
        """PyCantonese correctly tags 媽媽 as NOUN (not PROPN like Stanza)."""
        tagged = dict(pycantonese.pos_tag(pycantonese.segment("媽媽買咗好多嘢")))
        assert tagged["媽媽"] == "NOUN", f"媽媽 should be NOUN, got {tagged['媽媽']}"

    def test_overall_accuracy_above_80_percent(self) -> None:
        """PyCantonese POS accuracy on core Cantonese vocabulary exceeds 80%.

        Compare: Stanza zh-hans scores ~50% on the same vocabulary.
        """
        checks = [
            ("佢哋好鍾意食嘢", {"佢哋": "PRON", "鍾意": "VERB", "嘢": "NOUN"}),
            ("我想去買故事書", {"我": "PRON", "想": "AUX", "故事": "NOUN", "書": "NOUN"}),
            ("你知唔知道", {"你": "PRON", "唔": "ADV", "知道": "VERB"}),
            ("媽媽買咗好多嘢", {"媽媽": "NOUN", "咗": "PART", "嘢": "NOUN"}),
            ("佢係一個好人", {"佢": "PRON"}),
        ]

        total = 0
        correct = 0
        errors: list[tuple[str, str, str]] = []
        for sent, expected in checks:
            words = pycantonese.segment(sent)
            tagged = dict(pycantonese.pos_tag(words))
            for word, expected_pos in expected.items():
                total += 1
                actual = tagged.get(word, "MISSING")
                if actual == expected_pos:
                    correct += 1
                else:
                    errors.append((word, expected_pos, actual))

        accuracy = correct / total if total > 0 else 0
        assert accuracy > 0.80, (
            f"PyCantonese POS accuracy is {accuracy:.0%} ({correct}/{total}). "
            f"Expected >80%.\nErrors: {errors}"
        )


class TestPyCantonesePosOverride:
    """Test the POS override mechanism: PyCantonese POS replaces Stanza POS."""

    def test_override_replaces_stanza_pos_in_ud_words(self) -> None:
        """After Stanza runs, PyCantonese POS should override upos in UD output.

        This post-processing step runs for ALL Cantonese (lang=yue) morphotag,
        not just retokenize. Stanza produces UD words, then PyCantonese POS
        tags replace the upos field.
        """
        from batchalign.inference.morphosyntax import _override_pos_with_pycantonese

        # Simulate Stanza UD output with wrong Cantonese POS
        stanza_words = [
            {"id": (1,), "text": "佢哋", "upos": "PROPN", "lemma": "佢哋", "head": 3, "deprel": "nsubj"},
            {"id": (2,), "text": "好", "upos": "ADV", "lemma": "好", "head": 3, "deprel": "advmod"},
            {"id": (3,), "text": "鍾意", "upos": "VERB", "lemma": "鍾意", "head": 0, "deprel": "root"},
            {"id": (4,), "text": "食", "upos": "VERB", "lemma": "食", "head": 3, "deprel": "xcomp"},
            {"id": (5,), "text": "嘢", "upos": "PUNCT", "lemma": "嘢", "head": 3, "deprel": "punct"},
        ]

        result = _override_pos_with_pycantonese(stanza_words)

        # PyCantonese should fix the POS tags
        pos_map = {w["text"]: w["upos"] for w in result}
        assert pos_map["佢哋"] == "PRON", f"佢哋 should be PRON, got {pos_map['佢哋']}"
        assert pos_map["嘢"] == "NOUN", f"嘢 should be NOUN, got {pos_map['嘢']}"

        # Dependency parse should be preserved from Stanza (unchanged)
        dep_map = {w["text"]: w["deprel"] for w in result}
        assert dep_map["佢哋"] == "nsubj", "deprel should be preserved from Stanza"
        assert dep_map["鍾意"] == "root", "deprel should be preserved from Stanza"


    def test_override_works_on_presegmented_corpus_words(self) -> None:
        """PyCantonese POS override works on hand-transcribed corpus words.

        Words from the HKU aphasia corpus — already word-level, not per-char.
        This verifies the override works for ALL Cantonese morphotag, not just
        --retokenize ASR output.
        """
        from batchalign.inference.morphosyntax import _override_pos_with_pycantonese

        # Simulate Stanza UD output for corpus words with wrong POS
        stanza_words = [
            {"id": (1,), "text": "佢", "upos": "PROPN", "lemma": "佢", "head": 4, "deprel": "nsubj"},
            {"id": (2,), "text": "踢波", "upos": "VERB", "lemma": "踢波", "head": 0, "deprel": "root"},
            {"id": (3,), "text": "咗", "upos": "AUX", "lemma": "咗", "head": 2, "deprel": "aux"},
        ]

        result = _override_pos_with_pycantonese(stanza_words)
        pos_map = {w["text"]: w["upos"] for w in result}

        assert pos_map["佢"] == "PRON", f"佢 should be PRON, got {pos_map['佢']}"
        assert pos_map["咗"] == "PART", f"咗 should be PART, got {pos_map['咗']}"


class TestPyCantonesePosVsStanza:
    """Direct comparison of PyCantonese POS vs Stanza on same inputs."""

    @pytest.mark.golden
    def test_pycantonese_beats_stanza_on_cantonese(self) -> None:
        """PyCantonese POS accuracy exceeds Stanza zh-hans on Cantonese.

        This is the key test that justifies using PyCantonese instead of
        Stanza for Cantonese POS tagging.
        """
        import stanza

        nlp_stanza = stanza.Pipeline(
            lang="zh",
            processors="tokenize,pos",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=True,
        )

        checks = [
            ("佢哋 好 鍾意 食 嘢", {"佢哋": "PRON", "鍾意": "VERB", "嘢": "NOUN"}),
            ("你 知 唔 知道", {"你": "PRON", "唔": "ADV", "知道": "VERB"}),
            ("媽媽 買 咗 好多 嘢", {"媽媽": "NOUN", "咗": "PART", "嘢": "NOUN"}),
        ]

        stanza_correct = 0
        pyc_correct = 0
        total = 0

        for sent, expected in checks:
            # Stanza (pretokenized)
            stanza_doc = nlp_stanza(sent)
            stanza_pos = {w.text: w.upos for s in stanza_doc.sentences for w in s.words}

            # PyCantonese (segment + tag)
            words = sent.split()
            pyc_pos = dict(pycantonese.pos_tag(words))

            for word, expected_pos in expected.items():
                total += 1
                if stanza_pos.get(word) == expected_pos:
                    stanza_correct += 1
                if pyc_pos.get(word) == expected_pos:
                    pyc_correct += 1

        stanza_acc = stanza_correct / total
        pyc_acc = pyc_correct / total
        assert pyc_acc > stanza_acc, (
            f"PyCantonese ({pyc_acc:.0%}) should beat Stanza ({stanza_acc:.0%}) "
            f"on Cantonese POS accuracy"
        )
