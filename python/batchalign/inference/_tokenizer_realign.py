"""Tokenizer realignment for tokenize_no_ssplit mode.

When Stanza's neural tokenizer runs (``tokenize_no_ssplit=True``), it may
split compound words like "ice-cream" into multiple tokens ("ice", "-",
"cream").  This module provides a ``tokenize_postprocessor`` callback that
merges such spurious splits back, preserving the 1:1 mapping between CHAT
words and Stanza tokens.

MWT hints (True/False tuples)
------------------------------
Stanza's ``tokenize_postprocessor`` uses a tuple convention::

    (text, True)   — MWT: let the MWT processor expand (e.g. "don't" → do + n't)
    (text, False)  — NOT an MWT: suppress expansion (e.g. merged "ice-cream")
    plain string   — let Stanza's model decide (equivalent to model's own choice)

This module replicates Python master's ``tokenizer_processor`` logic:

* **Default (all languages)**: merged spurious splits → ``(text, False)``
  Prevents a merge like "ice-cream" from being expanded again by the MWT model.
* **English contractions**: merged text that contains ``'``, *unless* the
  prefix before the first ``'`` is ``"o"`` (e.g. o'clock, o'er) → ``(text, True)``
  Allows "don't", "Claus'" etc. to be handled by Stanza's MWT model.

This matches the Python master rules (``ud.py`` lines 680–685) exactly.

Thread safety: :class:`TokenizerContext` uses ``threading.local()`` to store
``original_words`` per-thread.  On free-threaded Python (3.14t+), multiple
threads can call ``nlp()`` concurrently without racing on the context.  On
regular Python, the ``nlp_lock`` in the batch callback still serializes
access, so the thread-local is effectively a single-thread property.
"""

from __future__ import annotations

import logging
import threading
from collections.abc import Callable
from typing import TypeAlias

L = logging.getLogger("batchalign")


TokenizerToken: TypeAlias = str | tuple[str, bool]


class TokenizerContext:
    """Thread-safe context shared between the batch callback and the postprocessor.

    Uses ``threading.local()`` so each thread's ``original_words`` is
    independent — required for free-threaded Python where multiple threads
    call ``nlp()`` concurrently on the same Pipeline.
    """

    def __init__(self) -> None:
        self._local = threading.local()

    @property
    def original_words(self) -> list[list[str]]:
        return getattr(self._local, "original_words", [])

    @original_words.setter
    def original_words(self, value: list[list[str]]) -> None:
        self._local.original_words = value


def make_tokenizer_postprocessor(
    ctx: TokenizerContext,
    alpha2: str = "",
) -> Callable[[list[list[TokenizerToken]]], list[list[TokenizerToken]]]:
    """Create a ``tokenize_postprocessor`` callback for ``stanza.Pipeline``.

    The returned callable has the signature Stanza expects::

        postprocessor(tokenized_batch: list[list]) -> list[list]

    where each inner list is the tokens for one sentence (paragraph).

    Parameters
    ----------
    ctx:
        Mutable context updated before each ``nlp()`` call with the original
        CHAT words for the current batch.
    alpha2:
        ISO-639-1 language code (e.g. ``"en"``, ``"fr"``).  Used to decide
        whether merged tokens should be flagged as MWT contractions.
    """

    def postprocessor(
        tokenized_batch: list[list[TokenizerToken]],
    ) -> list[list[TokenizerToken]]:
        if not ctx.original_words:
            return tokenized_batch

        result: list[list[TokenizerToken]] = []
        for sent_idx, sent_tokens in enumerate(tokenized_batch):
            if sent_idx < len(ctx.original_words):
                result.append(
                    _realign_sentence(sent_tokens, ctx.original_words[sent_idx], alpha2)
                )
            else:
                result.append(sent_tokens)
        return result

    return postprocessor


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _conform(token: TokenizerToken) -> str:
    """Extract text from a Stanza token (string or tuple)."""
    if isinstance(token, tuple):
        return str(token[0])
    return str(token)


def _is_contraction(text: str, alpha2: str) -> bool:
    """Return True if *text* should be flagged as an MWT contraction.

    Replicates Python master's ``tokenizer_processor`` English contraction rule
    (``ud.py`` lines 680–685)::

        (("en" in lang) and matches_in(i, "'") and
         not (len(conform(i).split("'")) > 1 and
              conform(i).split("'")[0].strip() == "o"))

    Returns ``True`` only for English tokens that contain an apostrophe and
    whose prefix before the first ``'`` is not ``"o"`` (which would be forms
    like o'clock, o'er, o'er the top, etc.).

    All other tokens return ``False``, meaning the MWT model will NOT try to
    expand them (suppresses spurious re-expansion of merged words).
    """
    if "'" not in text:
        return False
    if alpha2 != "en":
        return False
    # Exclude o'clock, o'er, o'er, etc. (prefix before first apostrophe is "o")
    parts = text.split("'")
    if len(parts) >= 2 and parts[0].strip().lower() == "o":
        return False
    return True


def _realign_sentence(
    stanza_tokens: list[TokenizerToken],
    original_words: list[str],
    alpha2: str = "",
) -> list[TokenizerToken]:
    """Merge Stanza tokens that map to the same original CHAT word.

    Delegates to ``batchalign_core.align_tokens()`` (Rust) for the
    character-position mapping algorithm. Per-language MWT override rules
    were retired on 2026-04-21 after a paired empirical audit — the
    character-DP alone satisfies the morphotag 1-to-1 invariant for all
    previously-patched languages.

    Stanza may return tokens with embedded spaces (rare edge case). These
    are flattened before passing to Rust so the character sequences match.

    Returned items may be plain strings or ``(text, bool)`` tuples, matching
    Stanza's postprocessor contract for MWT expansion hints.

    MWT hint preservation
    ---------------------
    Stanza's tokenizer natively emits ``(text, True)`` tuples for tokens
    it wants its MWT processor to expand (English contractions, French
    elisions, etc.). The Rust ``align_tokens`` function operates on raw
    strings for its char-DP algorithm, so tuple hints are temporarily
    erased when we flatten. When no merging actually happened (1:1
    mapping between Stanza tokens and the aligner's output), we overlay
    Stanza's original tuples back onto the aligned output so MWT
    expansion still fires downstream. Without this overlay, Stanza's MWT
    processor sees only plain strings and silently skips expansion — the
    direct cause of the 2026-04-13 Preserve-mode regression.
    """
    if not stanza_tokens or not original_words:
        return stanza_tokens

    # Flatten tokens that Stanza may have returned with embedded spaces
    flat_tokens: list[str] = []
    for tok in stanza_tokens:
        text = _conform(tok)
        parts = text.split(" ")
        flat_tokens.extend(parts if len(parts) > 1 else [text])

    from batchalign_core import align_tokens
    merged = align_tokens(original_words, flat_tokens, alpha2)

    # Restore Stanza's own MWT hint tuples wherever a merged/aligned token
    # still corresponds 1:1 to one original Stanza token. This matters even
    # when another token elsewhere in the sentence was merged (e.g.
    # "mm" + "-" + "hmm" -> "mm-hmm"): the unaffected English contraction
    # later in the sentence still needs its native (text, True) hint so the
    # MWT processor expands it.
    restored: list[TokenizerToken] = []
    orig_idx = 0
    for new in merged:
        target = _conform(new)
        start = orig_idx
        buf = ""
        while orig_idx < len(stanza_tokens) and len(buf) < len(target):
            buf += _conform(stanza_tokens[orig_idx])
            orig_idx += 1
            if buf == target:
                break

        # If we cannot reconcile the aligned token sequence back onto the
        # original Stanza sequence, return the aligner output unchanged.
        if buf != target:
            return merged

        if isinstance(new, tuple):
            restored.append(new)
            continue

        if (
            orig_idx - start == 1
            and isinstance(stanza_tokens[start], tuple)
            and _conform(stanza_tokens[start]) == target
        ):
            restored.append(stanza_tokens[start])
        else:
            restored.append(new)

    if orig_idx != len(stanza_tokens):
        return merged

    return restored
