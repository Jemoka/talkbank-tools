"""Stanza coreference inference: sentences -> coref chains.

Pure inference — no CHAT, no caching, no pipeline.
"""

from __future__ import annotations

import logging
import time

from pydantic import BaseModel, ValidationError

from batchalign.providers import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
)

L = logging.getLogger("batchalign.worker")


class CorefBatchItem(BaseModel):
    """A single item: one complete document as list of sentences."""

    sentences: list[list[str]]


class ChainRef(BaseModel):
    """A single coreference chain reference on a word.

    Matches Rust ``ChainRef`` in ``batchalign/src/coref.rs``.
    """

    chain_id: int
    is_start: bool
    is_end: bool


class CorefRawAnnotation(BaseModel):
    """Structured per-sentence coref data with typed chain references.

    Each element in ``words`` is parallel to the sentence's word list.
    Empty list means the word has no coreference chains.
    """

    sentence_idx: int
    words: list[list[ChainRef]]


class CorefRawResponse(BaseModel):
    """Raw structured coref response — Rust builds bracket notation from this."""

    annotations: list[CorefRawAnnotation]


def batch_infer_coref(req: BatchInferRequest) -> BatchInferResponse:
    """Batch Stanza coref inference: sentences -> CorefRawResponse.

    Each item is one complete document (list of sentences, each a list of words).
    Pipeline is lazily initialized and reused across documents in the batch.
    """
    t0 = time.monotonic()
    n = len(req.items)
    results: list[InferResponse] = []

    import stanza

    pipeline: stanza.Pipeline | None = None

    for item_idx, raw_item in enumerate(req.items):
        try:
            item = CorefBatchItem.model_validate(raw_item)
        except ValidationError:
            results.append(
                InferResponse(error="Invalid CorefBatchItem", elapsed_s=0.0)
            )
            continue

        if not item.sentences:
            results.append(
                InferResponse(
                    result=CorefRawResponse(annotations=[]).model_dump(),
                    elapsed_s=0.0,
                )
            )
            continue

        try:
            if pipeline is None:
                # Coref pipeline downloads its own model files (RoBERTa-large
                # plus the ontonotes coref package) on first use; surface the
                # wait through the progress channel.
                from batchalign.worker._progress import emit_download_event

                emit_download_event(
                    stage="downloading_stanza_coref_eng",
                    user_message=(
                        "Downloading Stanza English coreference model "
                        "(one-time, ~1.5 GB; future runs will use the local "
                        "cache)…"
                    ),
                )
                pipeline = stanza.Pipeline(
                    lang="en",
                    processors="tokenize, coref",
                    package={"coref": "ontonotes-singletons_roberta-large-lora"},
                    tokenize_pretokenized=True,
                )

            text = "\n\n".join(" ".join(s) for s in item.sentences)
            assert pipeline is not None
            result = pipeline(text)

            annotations: list[CorefRawAnnotation] = []
            sent_idx = 0
            for sent in result.sentences:
                if sent_idx >= len(item.sentences):
                    break

                word_chains: list[list[ChainRef]] = []
                has_coref = False

                for word in sent.words:
                    if word.coref_chains:
                        has_coref = True
                        refs: list[ChainRef] = [
                            ChainRef(
                                chain_id=chain.chain.index,
                                is_start=chain.is_start,
                                is_end=chain.is_end,
                            )
                            for chain in word.coref_chains
                        ]
                        word_chains.append(refs)
                    else:
                        word_chains.append([])

                if has_coref:
                    annotations.append(
                        CorefRawAnnotation(
                            sentence_idx=sent_idx,
                            words=word_chains,
                        )
                    )

                sent_idx += 1

            results.append(
                InferResponse(
                    result=CorefRawResponse(annotations=annotations).model_dump(),
                    elapsed_s=0.0,
                )
            )
        except Exception as e:
            L.warning("Coref infer failed for item %d: %s", item_idx, e)
            results.append(
                InferResponse(
                    result=CorefRawResponse(annotations=[]).model_dump(),
                    elapsed_s=0.0,
                )
            )

    elapsed = time.monotonic() - t0
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed
        )

    L.info("batch_infer coref: %d items, %.3fs", n, elapsed)
    return BatchInferResponse(results=results)
