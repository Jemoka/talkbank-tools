"""Stanza morphosyntax inference: words -> POS/dep/lemma.

Pure inference — no CHAT, no caching, no pipeline.
"""

from __future__ import annotations

import contextlib
import logging
import threading
import time
import unicodedata
from collections.abc import Callable, Iterator
from typing import TYPE_CHECKING

from pydantic import BaseModel, ValidationError, model_validator

from batchalign.inference._domain_types import LanguageCode

if TYPE_CHECKING:
    from batchalign.inference.types import StanzaNLP
    from batchalign.inference._tokenizer_realign import TokenizerContext

from batchalign.providers import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
    WorkerJSONValue,
)

L = logging.getLogger("batchalign.worker")


# ---------------------------------------------------------------------------
# Pydantic models
# ---------------------------------------------------------------------------

class MorphosyntaxBatchItem(BaseModel):
    """A single item in the batch morphosyntax payload from Rust."""

    words: list[str]
    terminator: str = "."
    special_forms: list[list[str | None]] = []
    lang: LanguageCode = ""


class UdWord(BaseModel, extra="allow"):
    """A single UD word/token — mirrors Rust ``UdWord`` in types.rs."""

    id: int | list[int] | float
    text: str
    lemma: str = ""
    upos: str = "X"
    xpos: str | None = None
    feats: str | None = None
    head: int = 0
    deprel: str = "dep"
    deps: str | None = None
    misc: str | None = None

    @model_validator(mode="after")
    def _default_lemma_to_text(self) -> UdWord:
        if not self.lemma and not isinstance(self.id, list):
            self.lemma = self.text
        return self

    @model_validator(mode="after")
    def _sanitize_pad_deprel(self) -> UdWord:
        if self.deprel.startswith("<") and self.deprel.endswith(">"):
            L.warning(
                "Stanza emitted deprel=%r for word %r — replacing with 'dep'",
                self.deprel,
                self.text,
            )
            self.deprel = "dep"
        return self


UdWordRaw = dict[str, str | int | float | list[int] | tuple[int, ...] | None]
JSONObject = dict[str, WorkerJSONValue]


# ---------------------------------------------------------------------------
# CJK word segmentation
# ---------------------------------------------------------------------------


def _segment_cantonese(words: list[str]) -> list[str]:
    """Segment Cantonese per-character tokens into words using PyCantonese.

    Only re-segments contiguous runs of single-CJK-character tokens.
    Existing multi-character tokens are preserved as-is to avoid breaking
    word boundaries that are already correct (e.g., from Tencent ASR or
    hand-transcribed corpora).

    This prevents the bug where joining all words into one string causes
    PyCantonese to merge tokens across word boundaries (e.g., 啦+飯+啦
    becoming 啦飯啦).
    """
    if not words:
        return []
    import pycantonese

    # Only re-segment if the input looks like per-character ASR output:
    # all CJK tokens are single characters. If any multi-char CJK token
    # exists, the input already has some word boundaries — preserve them.
    cjk_words = [w for w in words if any("\u4e00" <= c <= "\u9fff" for c in w)]
    has_multichar_cjk = any(len(w) > 1 for w in cjk_words)

    if has_multichar_cjk:
        # Input already has word boundaries — don't re-segment.
        # This prevents merging tokens across existing boundaries.
        return list(words)

    # All CJK tokens are single characters — safe to join and segment.
    text = "".join(words)
    if not text:
        return []
    return pycantonese.segment(text)


def _override_pos_with_pycantonese(
    ud_words: list[dict[str, object]],
) -> list[dict[str, object]]:
    """Override Stanza POS tags with PyCantonese POS for Cantonese words.

    Stanza's Mandarin-trained model misclassifies core Cantonese vocabulary
    (~50% accuracy). PyCantonese's POS tagger scores ~94% on the same words.
    This function replaces ``upos`` in each UD word dict while preserving
    all other fields (lemma, deprel, head, etc.) from Stanza.

    Called as a post-processing step when ``retokenize=True`` and ``lang=yue``.
    """
    import pycantonese

    texts = [w.get("text", "") for w in ud_words]
    if not texts:
        return ud_words

    tagged = pycantonese.pos_tag(texts)
    tag_map = {word: pos for word, pos in tagged}

    result = []
    for w in ud_words:
        text = w.get("text", "")
        pyc_pos = tag_map.get(text)
        if pyc_pos is not None:
            w = {**w, "upos": pyc_pos}
        result.append(w)
    return result


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------


def _is_bogus_lemma(text: str, lemma: str) -> bool:
    """Detect when Stanza returns a lemma that's pure punctuation for a word."""
    if text == lemma or not lemma:
        return False
    text_has_letters = any(unicodedata.category(c).startswith("L") for c in text)
    lemma_all_punct = all(
        unicodedata.category(c).startswith(("P", "S")) for c in lemma
    )
    return text_has_letters and lemma_all_punct


def validate_ud_words(sents: list[list[UdWordRaw]]) -> None:
    """Validate and normalize every token through the UdWord model.

    Mutates *sents* in place.
    """
    for sent in sents:
        for word_idx in range(len(sent)):
            raw = sent[word_idx]
            raw_id = raw.get("id")
            if isinstance(raw_id, tuple):
                raw["id"] = list(raw_id)

            validated = UdWord.model_validate(raw)

            if not isinstance(validated.id, list) and _is_bogus_lemma(
                validated.text, validated.lemma
            ):
                L.warning(
                    "Stanza returned bogus lemma %r for word %r — falling back to surface form",
                    validated.lemma,
                    validated.text,
                )
                validated.lemma = validated.text

            sent[word_idx] = validated.model_dump()


# ---------------------------------------------------------------------------
# Inference function
# ---------------------------------------------------------------------------


def batch_infer_morphosyntax(
    req: BatchInferRequest,
    nlp_pipelines: dict[LanguageCode, StanzaNLP],
    contexts: dict[LanguageCode, TokenizerContext],
    nlp_lock: threading.Lock,
    free_threaded: bool,
    mwt_lexicon: dict[str, list[str]] | None = None,
    progress_callback: Callable[[int, int], None] | None = None,
) -> BatchInferResponse:
    """Batch Stanza inference: (words, lang) -> UdResponse.

    Parameters
    ----------
    req : BatchInferRequest
        Batch of MorphosyntaxBatchItem payloads.
    nlp_pipelines : dict
        Pre-loaded Stanza Pipeline instances keyed by ISO-3 code.
    contexts : dict
        Tokenizer realignment contexts keyed by ISO-3 code.
    nlp_lock : threading.Lock
        Lock guarding Stanza calls on GIL-enabled Python.
    free_threaded : bool
        Whether to skip the lock (free-threaded Python).
    mwt_lexicon : dict, optional
        Custom multi-word token lexicon mapping surface forms to
        expansion tokens (e.g. ``{"gonna": ["going", "to"]}``).
        When provided, matching tokens in Stanza's output are
        expanded according to this lexicon.
    """

    @contextlib.contextmanager
    def _maybe_lock() -> Iterator[None]:
        if free_threaded:
            yield
        else:
            with nlp_lock:
                yield

    t0 = time.monotonic()

    n = len(req.items)
    items: list[MorphosyntaxBatchItem | None] = []
    for raw_item in req.items:
        try:
            items.append(MorphosyntaxBatchItem.model_validate(raw_item))
        except ValidationError:
            items.append(None)

    empty_ud: JSONObject = {"sentences": []}
    results: list[InferResponse] = [
        InferResponse(result=empty_ud, elapsed_s=0.0) for _ in range(n)
    ]

    by_lang: dict[str, list[tuple[int, str, list[str]]]] = {}
    for i, item in enumerate(items):
        if item is None:
            results[i] = InferResponse(error="Invalid batch item", elapsed_s=0.0)
            continue
        if not item.words:
            continue

        words = list(item.words)
        item_lang = item.lang or req.lang

        # Apply PyCantonese word segmentation for Cantonese retokenize
        if req.retokenize and item_lang in ("yue",):
            words = _segment_cantonese(words)

        # Join words for Stanza input. Do NOT strip parentheses here —
        # Rust cleaned_text() already handles CHAT notation. Stripping
        # parens in Python silently drops bare "(" / ")" words, causing
        # MOR count mismatches in the retokenize inject path.
        text = " ".join(words).strip()

        if item_lang not in by_lang:
            by_lang[item_lang] = []
        by_lang[item_lang].append((i, text, words))

    if not by_lang:
        return BatchInferResponse(results=results)

    for lang_code, lang_items in by_lang.items():
        indices = [idx for idx, _, _ in lang_items]
        texts = [text for _, text, _ in lang_items]
        word_lists = [words for _, _, words in lang_items]


        # Mandarin retokenize: use Stanza neural tokenizer instead of pretokenized.
        # Only activate when the JOB language is Mandarin — per-utterance language
        # codes (e.g., [- zho] in a Cantonese file) must NOT trigger retokenization.
        use_retok_pipeline = (
            req.retokenize
            and lang_code in ("zho", "cmn")
            and req.lang in ("zho", "cmn")
        )
        if use_retok_pipeline:
            retok_key = f"{lang_code}:retok"
            nlp = nlp_pipelines.get(retok_key)
            if nlp is None:
                # Lazy-load the retokenize pipeline on first request
                from batchalign.worker._stanza_loading import load_stanza_retokenize_model

                load_stanza_retokenize_model(lang_code)
                nlp = nlp_pipelines.get(retok_key)
            if nlp is None:
                L.warning(
                    "Failed to load retokenize pipeline for %s", lang_code,
                )
                use_retok_pipeline = False
        if not use_retok_pipeline:
            nlp = nlp_pipelines.get(lang_code)
        if nlp is None:
            L.warning(
                "No Stanza pipeline for language %s -- items will have empty UdResponse",
                lang_code,
            )
            continue

        # For Mandarin retokenize, join with spaces. Stanza's neural tokenizer
        # (tokenize_pretokenized=False) handles re-segmentation regardless of
        # spacing. Using no-space join ("".join) would merge Latin+CJK words
        # (e.g., "hello你好" → one token) in code-switched utterances.
        if use_retok_pipeline:
            combined = "\n\n".join(" ".join(w) for w in word_lists)
        else:
            combined = "\n\n".join(texts)
        if use_retok_pipeline:
            retok_key = f"{lang_code}:retok"
            tok_ctx = contexts.get(retok_key) or contexts.get(lang_code) or contexts.get(req.lang)
        else:
            tok_ctx = contexts.get(lang_code) or contexts.get(req.lang)

        try:
            with _maybe_lock():
                # Set original_words so the postprocessor can realign Stanza's
                # tokenization back to CHAT words. Skip when retokenize is
                # requested (either CJK retok pipeline or non-CJK retokenize)
                # — in that case, Stanza owns tokenization and we want its
                # MWT expansion (gonna → gon+na, don't → do+n't) to pass through.
                should_set_original_words = (
                    tok_ctx is not None
                    and not use_retok_pipeline
                    and not req.retokenize
                )
                # Realignment-skipped audit (Wave 3 of the morphotag
                # reconciliation architecture): if we're in normal
                # (non-retok) mode and tok_ctx is None, Stanza runs
                # WITHOUT tokenizer realignment — its neural tokenizer
                # is free to split/merge CHAT words, silently breaking
                # the 1-to-1 invariant that downstream Rust injection
                # assumes. This warning makes that invisible mode
                # visible so we can fix the realignment context wiring
                # rather than paper over it with post-hoc alignment.
                # See `book/src/architecture/morphotag-invariants.md`.
                realignment_skipped_unexpectedly = (
                    tok_ctx is None
                    and not use_retok_pipeline
                    and not req.retokenize
                )
                if realignment_skipped_unexpectedly:
                    L.warning(
                        "morphotag: realignment context missing for language "
                        "%r (lookup keys tried: %r, %r) — Stanza will own "
                        "tokenization on this batch, which may violate the "
                        "1-to-1 invariant. This batch's count mismatches "
                        "will surface as MisalignmentBug decisions.",
                        lang_code,
                        lang_code,
                        req.lang,
                    )
                if should_set_original_words:
                    tok_ctx.original_words = word_lists
                doc = nlp(combined)
                if should_set_original_words:
                    tok_ctx.original_words = []

            sents = doc.to_dict()

            if len(sents) != len(indices):
                L.warning(
                    "Stanza sentence count mismatch for language %s (expected %d, got %d)",
                    lang_code,
                    len(indices),
                    len(sents),
                )
            else:
                # For Cantonese, override Stanza POS with PyCantonese.
                # Stanza's Mandarin model scores ~50% on Cantonese vocabulary;
                # PyCantonese scores ~94%. We keep Stanza's dependency parse
                # (deprel, head) and lemma — only upos is replaced.
                # Applied to ALL Cantonese morphotag, not just retokenize,
                # because the POS accuracy problem affects all Cantonese output.
                apply_pyc_pos = lang_code in ("yue",)

                for i, idx in enumerate(indices):
                    sent = sents[i]
                    if apply_pyc_pos:
                        sent = _override_pos_with_pycantonese(sent)
                    results[idx] = InferResponse(
                        result={"raw_sentences": [sent]},
                        elapsed_s=0.0,
                    )
        except Exception as e:
            L.warning(
                "Stanza batch failed for language %s (%d items): %s",
                lang_code,
                len(indices),
                e,
            )
            if tok_ctx is not None:
                tok_ctx.original_words = []

        # Report progress: how many items have been processed so far
        # (across all language groups).
        if progress_callback is not None:
            completed_so_far = sum(
                1 for r in results if r.result != empty_ud or r.error is not None
            )
            progress_callback(completed_so_far, n)

    elapsed = time.monotonic() - t0
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed
        )

    L.info("batch_infer morphosyntax: %d items, %.3fs", n, elapsed)
    return BatchInferResponse(results=results)
