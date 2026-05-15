"""Tests for _tokenizer_realign.py — MWT hint annotation and merge logic.

Verifies that merged tokens are returned as (text, bool) MWT hint tuples
matching Python master's tokenizer_processor rules exactly:
  - English tokens containing ' (except o' forms) → (text, True)
  - All other merges → (text, False)
"""

from __future__ import annotations

from batchalign.inference._tokenizer_realign import (
    _conform,
    _is_contraction,
    _realign_sentence,
    TokenizerContext,
    make_tokenizer_postprocessor,
)


# ---------------------------------------------------------------------------
# _is_contraction
# ---------------------------------------------------------------------------

class TestIsContraction:
    """Replicates Python master ud.py lines 680–685 logic."""

    # --- English contractions → True ---

    def test_dont_is_contraction(self) -> None:
        assert _is_contraction("don't", "en") is True

    def test_cant_is_contraction(self) -> None:
        assert _is_contraction("can't", "en") is True

    def test_wont_is_contraction(self) -> None:
        assert _is_contraction("won't", "en") is True

    def test_its_is_contraction(self) -> None:
        # "it's" — contraction of "it is"
        assert _is_contraction("it's", "en") is True

    def test_possessive_is_contraction(self) -> None:
        # English possessives ("Claus'", "John's") also match Python master's rule
        assert _is_contraction("Claus'", "en") is True
        assert _is_contraction("John's", "en") is True

    def test_im_is_contraction(self) -> None:
        assert _is_contraction("I'm", "en") is True

    # --- English o' forms → False (excluded) ---

    def test_oclock_excluded(self) -> None:
        assert _is_contraction("o'clock", "en") is False

    def test_oer_excluded(self) -> None:
        assert _is_contraction("o'er", "en") is False

    # --- Non-English languages → False ---

    def test_french_contraction_is_not_mwt(self) -> None:
        # French "l'" would be a clitic, but we return False for non-English
        assert _is_contraction("l'", "fr") is False

    def test_dutch_possessive_is_not_mwt(self) -> None:
        assert _is_contraction("'s", "nl") is False

    def test_spanish_is_not_mwt(self) -> None:
        assert _is_contraction("it's", "es") is False

    def test_empty_alpha2_is_not_mwt(self) -> None:
        assert _is_contraction("don't", "") is False

    # --- No apostrophe → False ---

    def test_plain_word_is_not_mwt(self) -> None:
        assert _is_contraction("hello", "en") is False

    def test_hyphenated_is_not_mwt(self) -> None:
        assert _is_contraction("ice-cream", "en") is False


# ---------------------------------------------------------------------------
# _realign_sentence — merged token tuples
# ---------------------------------------------------------------------------

class TestRealignSentenceMwtTuples:
    """Verify merged tokens become (text, bool) tuples with correct MWT hint."""

    def test_spurious_split_becomes_false_tuple(self) -> None:
        """Non-English/non-apostrophe merge → (text, False)."""
        # Stanza split "ice-cream" into ["ice", "-", "cream"]
        tokens = ["ice", "-", "cream"]
        words = ["ice-cream"]
        result = _realign_sentence(tokens, words, alpha2="en")
        assert result == [("ice-cream", False)]

    def test_english_contraction_split_becomes_true_tuple(self) -> None:
        """English apostrophe merge → (text, True)."""
        # Stanza split "don't" into ["don", "'t"] — maps to one word "don't"
        tokens = ["don", "'t"]
        words = ["don't"]
        result = _realign_sentence(tokens, words, alpha2="en")
        assert result == [("don't", True)]

    def test_possessive_becomes_true_tuple(self) -> None:
        """English possessive merge → (text, True)."""
        tokens = ["Claus", "'"]
        words = ["Claus'"]
        result = _realign_sentence(tokens, words, alpha2="en")
        assert result == [("Claus'", True)]

    def test_french_apostrophe_becomes_false_tuple(self) -> None:
        """Non-English apostrophe merge → (text, False)."""
        tokens = ["l", "'"]
        words = ["l'"]
        result = _realign_sentence(tokens, words, alpha2="fr")
        assert result == [("l'", False)]

    def test_single_token_unchanged(self) -> None:
        """Single-token words pass through unchanged (no merge, no tuple)."""
        tokens = ["hello", "world"]
        words = ["hello", "world"]
        result = _realign_sentence(tokens, words, alpha2="en")
        # 1:1 mapping → fast path, tokens unchanged
        assert result == ["hello", "world"]

    def test_mixed_merge_and_single(self) -> None:
        """Merged tokens get tuple; single tokens stay as-is."""
        # "ice-cream" split, "good" and "bye" each single
        tokens = ["ice", "-", "cream", "good", "bye"]
        words = ["ice-cream", "good", "bye"]
        result = _realign_sentence(tokens, words, alpha2="en")
        assert result[0] == ("ice-cream", False)
        assert _conform(result[1]) == "good"
        assert _conform(result[2]) == "bye"

    def test_merge_elsewhere_does_not_drop_later_mwt_hint(self) -> None:
        """A preceding merge must not strip a later unaffected MWT tuple.

        Regression for the English preserve-mode defect on:
        ``mm-hmm that's right .``

        Stanza tokenizes this as:
        - ``mm``, ``-``, ``hmm``  (spurious split that must be merged)
        - ``(\"that's\", True)``   (native MWT hint that must survive)
        """
        tokens = ["mm", "-", "hmm", ("that's", True), "right", "."]
        words = ["mm-hmm", "that's", "right", "."]
        result = _realign_sentence(tokens, words, alpha2="en")
        assert result[0] == ("mm-hmm", False)
        assert result[1] == ("that's", True)
        assert _conform(result[2]) == "right"
        assert _conform(result[3]) == "."

    def test_no_alpha2_defaults_to_false(self) -> None:
        """No language → False for apostrophe merges (safe default)."""
        tokens = ["don", "'t"]
        words = ["don't"]
        result = _realign_sentence(tokens, words)  # alpha2 defaults to ""
        assert result == [("don't", False)]

    def test_character_mismatch_returns_unchanged(self) -> None:
        """When chars don't match, return stanza tokens unmodified."""
        tokens = ["totally", "different"]
        words = ["something", "else"]
        result = _realign_sentence(tokens, words, alpha2="en")
        # No merge attempt — returned as-is
        assert result == tokens


# ---------------------------------------------------------------------------
# make_tokenizer_postprocessor — alpha2 threading
# ---------------------------------------------------------------------------

class TestMakeTokenizerPostprocessor:
    """Verify alpha2 is captured in the closure and applied correctly."""

    def test_alpha2_english_contraction_tuple(self) -> None:
        ctx = TokenizerContext()
        ctx.original_words = [["don't"]]
        pp = make_tokenizer_postprocessor(ctx, alpha2="en")
        batch = [["don", "'t"]]
        result = pp(batch)  # type: ignore[operator]
        # Should be (don't, True) — English contraction
        assert result[0][0] == ("don't", True)

    def test_alpha2_non_english_no_contraction(self) -> None:
        ctx = TokenizerContext()
        ctx.original_words = [["l'"]]
        pp = make_tokenizer_postprocessor(ctx, alpha2="fr")
        batch = [["l", "'"]]
        result = pp(batch)  # type: ignore[operator]
        assert result[0][0] == ("l'", False)

    def test_empty_context_returns_batch_unchanged(self) -> None:
        ctx = TokenizerContext()
        # No original_words set
        pp = make_tokenizer_postprocessor(ctx, alpha2="en")
        batch = [["hello", "world"]]
        result = pp(batch)  # type: ignore[operator]
        assert result is batch
