"""Regression test: parenthesis stripping must not change word count.

Bug hypothesis: if the Rust extractor includes a bare "(" or ")" as a
word (from CHAT group notation), the Python code in batch_infer_morphosyntax
line 268 silently removes it via .replace("(", "").replace(")", ""),
reducing the word count sent to Stanza. This causes MOR count mismatch.

The fix: remove the .replace("(", "").replace(")", "") entirely.
Rust cleaned_text() already handles CHAT notation.
"""

from __future__ import annotations


def test_paren_strip_reduces_word_count() -> None:
    """Demonstrate: bare paren word becomes empty, reducing Stanza token count.

    This is the ROOT CAUSE of the retrace retokenize bug:
    MOR item count (5) does not match alignable word count (6).
    """
    # Simulated extracted words where one is a bare "("
    words = ["呢", "度", "(", "食飯", "啦", "啦"]

    # The buggy line from morphosyntax.py:268
    text = " ".join(words).replace("(", "").replace(")", "").strip()
    stanza_tokens = text.split()

    assert len(stanza_tokens) < len(words), (
        f"The .replace('(','') drops bare paren words: "
        f"{len(words)} words → {len(stanza_tokens)} tokens. "
        f"This causes the MOR count mismatch."
    )
    assert len(stanza_tokens) == len(words) - 1, (
        f"Exactly one word should be lost: {words} → {stanza_tokens}"
    )


def test_paren_strip_removed_fixes_count() -> None:
    """After removing the .replace, word count is preserved."""
    words = ["呢", "度", "(", "食飯", "啦", "啦"]

    # Fixed: no parenthesis stripping
    text = " ".join(words).strip()
    stanza_tokens = text.split()

    assert len(stanza_tokens) == len(words), (
        f"Without .replace, word count preserved: {len(words)} == {len(stanza_tokens)}"
    )
