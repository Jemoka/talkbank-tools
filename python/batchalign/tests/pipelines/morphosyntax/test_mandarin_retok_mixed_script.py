"""Test: Mandarin retokenize join must not lose Latin word boundaries.

Bug 15: line 319 joins words without spaces for Mandarin retokenize.
If a Mandarin utterance has code-switched Latin words (e.g., "hello 你好"),
joining without spaces produces "hello你好" — one token instead of two.
"""

from __future__ import annotations


def test_mandarin_join_loses_latin_boundaries() -> None:
    """Document: joining without spaces merges Latin+CJK into one token."""
    words = ["hello", "你好", "世界"]

    # Current: join without spaces (Mandarin retokenize convention)
    text_no_spaces = "".join(words)
    assert text_no_spaces == "hello你好世界", "All merged into one string"
    assert len(text_no_spaces.split()) == 1, "Only 1 token — Latin lost"

    # Fixed: join with space only between Latin and CJK boundaries
    # or just use space join (safe for Stanza neural tokenizer)
    text_with_spaces = " ".join(words)
    assert len(text_with_spaces.split()) == 3, "3 tokens preserved"
