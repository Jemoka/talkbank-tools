"""Contract test for morphotag tokenizer realignment (Wave 3).

The morphotag pipeline's 1-to-1 invariant — that Stanza returns exactly N
tokens for N CHAT words after tokenizer realignment — depends on
``batch_infer_morphosyntax`` setting ``tok_ctx.original_words`` to the
batch's word lists BEFORE invoking ``nlp()``. If that sequencing breaks
(e.g. a refactor moves the assignment after the call, or a new branch
skips the assignment for non-retok mode), Stanza's neural tokenizer
runs without realignment and the invariant is silently violated.

This test locks the contract using a fake NLP callable that records what
``tok_ctx.original_words`` held at the moment it was invoked. The real
behavior we are testing lives in
``python/batchalign/inference/morphosyntax.py::batch_infer_morphosyntax``
around lines 343-378 (the ``should_set_original_words`` block).

See ``book/src/architecture/morphotag-invariants.md`` for the
architectural rationale.
"""

from __future__ import annotations

import threading
from typing import Any

from batchalign.inference._tokenizer_realign import TokenizerContext
from batchalign.inference.morphosyntax import batch_infer_morphosyntax
from batchalign.worker._types import BatchInferRequest, InferTask


class _FakeDoc:
    """Minimal stand-in for a ``stanza.Document``."""

    def __init__(self, sentences: list[dict[str, Any]]) -> None:
        self._sentences = sentences

    def to_dict(self) -> list[dict[str, Any]]:
        return self._sentences


def _make_ud_sentence(words: list[str]) -> dict[str, Any]:
    """Build a minimal dict matching Stanza's ``to_dict()[0]`` shape."""
    return {
        "tokens": [
            {"id": i + 1, "text": w}
            for i, w in enumerate(words)
        ],
        "words": [
            {
                "id": i + 1,
                "text": w,
                "lemma": w,
                "upos": "NOUN",
                "xpos": None,
                "feats": None,
                "head": 0 if i == 0 else 1,
                "deprel": "root" if i == 0 else "dep",
                "misc": None,
                "start_char": None,
                "end_char": None,
            }
            for i, w in enumerate(words)
        ],
    }


def test_realignment_context_is_set_before_nlp_in_normal_mode() -> None:
    """In non-retok mode, ``tok_ctx.original_words`` must be populated
    with the request's per-item word lists at the moment ``nlp()`` is
    called. This is the primary invariant that keeps Stanza's tokenizer
    aligned to CHAT word boundaries."""

    ctx = TokenizerContext()

    # Record the state of ``ctx.original_words`` at the exact moment
    # ``nlp()`` is invoked.
    observed_original_words: list[list[list[str]]] = []

    def fake_nlp(_text: str) -> _FakeDoc:
        observed_original_words.append(list(ctx.original_words))
        # One sentence per item in the batch, each with the batched words.
        return _FakeDoc([_make_ud_sentence(["hello"]), _make_ud_sentence(["world"])])

    request = BatchInferRequest(
        task=InferTask.MORPHOSYNTAX,
        lang="eng",
        items=[
            {
                "words": ["hello"],
                "terminator": ".",
                "special_forms": [[None, None]],
                "lang": "eng",
            },
            {
                "words": ["world"],
                "terminator": ".",
                "special_forms": [[None, None]],
                "lang": "eng",
            },
        ],
        mwt={},
        retokenize=False,
    )

    batch_infer_morphosyntax(
        request,
        nlp_pipelines={"eng": fake_nlp},  # type: ignore[dict-item]
        contexts={"eng": ctx},
        nlp_lock=threading.Lock(),
        free_threaded=False,
    )

    # nlp() must have been called at least once.
    assert observed_original_words, (
        "nlp() was never invoked — the batch collection or language "
        "dispatch skipped the request, which would make the realignment "
        "invariant moot but also means morphotag produced no output."
    )

    # At the moment nlp() was invoked, ctx.original_words MUST be the
    # word_lists corresponding to the batch items for this language.
    # The exact ordering follows the request items for the dispatch
    # language.
    assert observed_original_words[0] == [["hello"], ["world"]], (
        f"tok_ctx.original_words was {observed_original_words[0]!r} at "
        f"the moment nlp() was invoked, but the invariant requires it "
        f"to carry the batch's word lists so Stanza can realign to CHAT "
        f"boundaries. If this assertion fires, the realignment sequencing "
        f"in batch_infer_morphosyntax has drifted — Stanza will tokenize "
        f"freely and the 1-to-1 invariant will silently break."
    )


def test_realignment_context_cleared_after_nlp_call() -> None:
    """After ``nlp()`` returns, ``tok_ctx.original_words`` must be reset
    to the empty list, so a subsequent call that forgets to set it (or
    a different thread sharing the context) cannot accidentally use
    stale word lists."""

    ctx = TokenizerContext()

    def fake_nlp(_text: str) -> _FakeDoc:
        return _FakeDoc([_make_ud_sentence(["hello"])])

    request = BatchInferRequest(
        task=InferTask.MORPHOSYNTAX,
        lang="eng",
        items=[
            {
                "words": ["hello"],
                "terminator": ".",
                "special_forms": [[None, None]],
                "lang": "eng",
            },
        ],
        mwt={},
        retokenize=False,
    )

    batch_infer_morphosyntax(
        request,
        nlp_pipelines={"eng": fake_nlp},  # type: ignore[dict-item]
        contexts={"eng": ctx},
        nlp_lock=threading.Lock(),
        free_threaded=False,
    )

    # After the call, the context must be cleared to the empty default.
    # Any later call in normal mode that sets original_words will get
    # the fresh value; this guard protects against stale leakage if a
    # future code path forgets to set it and unknowingly inherits the
    # previous batch's words.
    assert ctx.original_words == [], (
        f"tok_ctx.original_words leaked past the nlp() call: got "
        f"{ctx.original_words!r}. The cleanup branch at "
        f"morphosyntax.py:~378 is supposed to reset it to []."
    )


def test_no_realignment_when_retokenize_requested() -> None:
    """When ``req.retokenize`` is True (generic retokenize), the caller
    has explicitly asked Stanza to own tokenization, so the pipeline
    MUST NOT set ``tok_ctx.original_words`` — doing so would fight
    Stanza's tokenizer and likely produce incorrect MWT output.

    This test locks the opposite contract: in retokenize mode,
    ``ctx.original_words`` stays empty (i.e. the default) when nlp()
    is invoked.
    """

    ctx = TokenizerContext()

    observed: list[list[list[str]]] = []

    def fake_nlp(_text: str) -> _FakeDoc:
        observed.append(list(ctx.original_words))
        return _FakeDoc([_make_ud_sentence(["hello"])])

    request = BatchInferRequest(
        task=InferTask.MORPHOSYNTAX,
        lang="eng",
        items=[
            {
                "words": ["hello"],
                "terminator": ".",
                "special_forms": [[None, None]],
                "lang": "eng",
            },
        ],
        mwt={},
        retokenize=True,
    )

    batch_infer_morphosyntax(
        request,
        nlp_pipelines={"eng": fake_nlp},  # type: ignore[dict-item]
        contexts={"eng": ctx},
        nlp_lock=threading.Lock(),
        free_threaded=False,
    )

    assert observed, "nlp() was not invoked"
    assert observed[0] == [], (
        f"In retokenize mode, tok_ctx.original_words must be empty "
        f"at nlp() call time so Stanza's tokenizer is free to produce "
        f"its own segmentation (including MWT expansion). Got "
        f"{observed[0]!r}. If this fires, the retokenize branch is "
        f"incorrectly populating the realignment context."
    )
