"""Stanza POS tagging accuracy on Cantonese vocabulary.

This test measures how well Stanza's Mandarin-trained zh model handles
Cantonese-specific vocabulary when used for morphotag. This affects ALL
Cantonese morphotag output — not just the new --retokenize feature.

batchalign3 (inherited from batchalign2) maps Cantonese (yue) to Stanza's
Chinese (zh) model, which is trained on the Chinese Treebank (Mandarin
formal text). Cantonese has different pronouns, negation particles, copula,
and vocabulary that the Mandarin model may misclassify.

Finding (2026-03-23): 50% POS accuracy on core Cantonese vocabulary.
Systematic errors include:
- 佢/佢哋 (he/they): PRON → PROPN (unknown Cantonese pronoun)
- 嘢 (thing/stuff): NOUN → PUNCT (completely wrong)
- 唔 (not): ADV → VERB (unknown Cantonese negation)
- 係 (is/be): AUX → VERB (unknown Cantonese copula)
"""

from __future__ import annotations

import pytest


@pytest.mark.golden
class TestStanzaCantonesePos:
    """Measure Stanza zh model POS accuracy on Cantonese vocabulary.

    Uses real Stanza model inference — not fakes. Marked golden because
    it loads the Stanza Chinese model (~200 MB).
    """

    @staticmethod
    def _get_pos_map(nlp, sentence: str) -> dict[str, str]:
        """Run Stanza and return {word: UPOS} map."""
        doc = nlp(sentence)
        return {w.text: w.upos for s in doc.sentences for w in s.words}

    @staticmethod
    def _load_stanza():
        import stanza
        return stanza.Pipeline(
            lang="zh",
            processors="tokenize,pos,lemma,depparse",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=True,
        )

    def test_cantonese_pronouns_misclassified(self) -> None:
        """Stanza classifies Cantonese pronouns 佢/佢哋 as PROPN, not PRON.

        Mandarin uses 他/他們 for he/they. 佢/佢哋 are Cantonese-specific
        and absent from the Mandarin training data.
        """
        nlp = self._load_stanza()

        pos = self._get_pos_map(nlp, "佢 係 好 人")
        assert pos["佢"] == "PROPN", (
            f"Expected Stanza to misclassify 佢 as PROPN (not PRON), got {pos['佢']}. "
            "If this changed, Stanza may have improved its Cantonese coverage."
        )

        pos2 = self._get_pos_map(nlp, "佢哋 好 鍾意 食 嘢")
        assert pos2["佢哋"] == "PROPN", (
            f"Expected Stanza to misclassify 佢哋 as PROPN, got {pos2['佢哋']}."
        )

    def test_cantonese_negation_misclassified(self) -> None:
        """Stanza classifies Cantonese negation 唔 as VERB, not ADV.

        Mandarin uses 不/没 for negation. 唔 is Cantonese-specific.
        """
        nlp = self._load_stanza()
        pos = self._get_pos_map(nlp, "你 知 唔 知道")
        assert pos["唔"] != "ADV", (
            f"唔 was correctly classified as ADV — Stanza may have improved. "
            f"Got {pos['唔']}."
        )

    def test_cantonese_ye_misclassified(self) -> None:
        """Stanza classifies Cantonese 嘢 (thing/stuff) as PUNCT or PART, not NOUN.

        嘢 is a high-frequency Cantonese noun meaning 'thing/stuff'. It does
        not exist in Mandarin. Stanza treats it as punctuation or particle.
        """
        nlp = self._load_stanza()
        pos = self._get_pos_map(nlp, "佢哋 好 鍾意 食 嘢")
        assert pos["嘢"] != "NOUN", (
            f"嘢 was correctly classified as NOUN — Stanza may have improved. "
            f"Got {pos['嘢']}."
        )

    def test_overall_accuracy_below_60_percent(self) -> None:
        """Stanza's overall POS accuracy on Cantonese core vocabulary is poor.

        This test documents the measured accuracy. If it starts passing at
        >60%, Stanza has improved and we should re-evaluate whether a
        Cantonese-specific solution is still needed for POS tagging.
        """
        nlp = self._load_stanza()

        checks = [
            ("佢哋 好 鍾意 食 嘢", {"佢哋": "PRON", "鍾意": "VERB", "嘢": "NOUN"}),
            ("我 想 去 買 故事 書", {"我": "PRON", "想": "AUX", "故事": "NOUN", "書": "NOUN"}),
            ("你 知 唔 知道", {"你": "PRON", "唔": "ADV", "知道": "VERB"}),
            ("媽媽 買 咗 好多 嘢", {"媽媽": "NOUN", "咗": "PART", "好多": "ADJ", "嘢": "NOUN"}),
            ("佢 係 一 個 好 人", {"佢": "PRON", "係": "AUX", "好": "ADJ", "人": "NOUN"}),
        ]

        total = 0
        correct = 0
        errors: list[tuple[str, str, str]] = []
        for sent, expected in checks:
            pos = self._get_pos_map(nlp, sent)
            for word, expected_pos in expected.items():
                total += 1
                actual = pos.get(word, "MISSING")
                if actual == expected_pos:
                    correct += 1
                else:
                    errors.append((word, expected_pos, actual))

        accuracy = correct / total if total > 0 else 0
        assert accuracy < 0.60, (
            f"Stanza Cantonese POS accuracy is {accuracy:.0%} ({correct}/{total}). "
            f"If >60%, Stanza has improved and this test should be updated.\n"
            f"Errors: {errors}"
        )

    def test_mandarin_equivalents_are_correct(self) -> None:
        """The same sentences in Mandarin get correct POS tags.

        This proves the issue is Cantonese-specific vocabulary, not a
        general Stanza quality problem.
        """
        nlp = self._load_stanza()

        # Mandarin equivalents of the Cantonese test sentences
        pos = self._get_pos_map(nlp, "他们 很 喜欢 吃 东西")
        assert pos["他们"] == "PRON", f"Mandarin 他们 should be PRON, got {pos['他们']}"
        assert pos["喜欢"] == "VERB", f"Mandarin 喜欢 should be VERB, got {pos['喜欢']}"

        pos2 = self._get_pos_map(nlp, "你 知 不 知道")
        assert pos2["不"] == "ADV", f"Mandarin 不 should be ADV, got {pos2['不']}"

    def test_zh_hant_also_wrong_for_cantonese(self) -> None:
        """batchalignHK used zh-hant (Traditional Chinese) instead of zh-hans.

        Neither is Cantonese-specific. zh-hant uses the GSD treebank (Traditional),
        zh-hans uses GSD-Simplified. Both are Mandarin-trained. This test verifies
        that switching to zh-hant does NOT fix the Cantonese POS problem.
        """
        import stanza

        nlp_hant = stanza.Pipeline(
            lang="zh-hant",
            processors="tokenize,pos,lemma,depparse",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=True,
        )
        pos = self._get_pos_map(nlp_hant, "佢哋 好 鍾意 食 嘢")

        # zh-hant classifies 佢哋 as VERB (even worse than zh-hans PROPN)
        assert pos["佢哋"] != "PRON", (
            f"If zh-hant now correctly tags 佢哋 as PRON, re-evaluate model choice. "
            f"Got {pos['佢哋']}."
        )

        # 嘢 still wrong in zh-hant
        assert pos["嘢"] != "NOUN", (
            f"If zh-hant now correctly tags 嘢 as NOUN, re-evaluate model choice. "
            f"Got {pos['嘢']}."
        )
