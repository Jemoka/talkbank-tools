"""Cantonese FA inference provider for built-in HK/Cantonese engines.

New-style provider: a ``load`` function (called once at worker startup)
and an ``infer`` function (called per batch_infer request).

Wraps the standard Wave2Vec MMS forced alignment model with a Cantonese
preprocessing step: hanzi characters are converted to jyutping romanization
(tone-stripped, syllables joined with apostrophes) before alignment.  For
non-``yue`` languages, words pass through unchanged.
"""

from __future__ import annotations

from dataclasses import dataclass
import logging
import re
import threading
import time
from typing import TYPE_CHECKING, Protocol

from pydantic import ValidationError

from batchalign.worker._types import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
)
from batchalign.inference.fa import (
    FaIndexedTiming,
    FaInferItem,
    Wave2VecIndexedResponse,
)

from batchalign.inference._domain_types import LanguageCode

from ._common import EngineOverrides

if TYPE_CHECKING:
    from batchalign.inference.audio import ASRAudioFile
    from batchalign.inference.types import Wave2VecFAHandle

L = logging.getLogger("batchalign.hk.fa")


# ---------------------------------------------------------------------------
# Protocol for pycantonese (avoids importing the heavy module at type level)
# ---------------------------------------------------------------------------


class _PyCantonese(Protocol):
    def characters_to_jyutping(
        self, text: str
    ) -> list[tuple[str, str | None]]: ...


class _AudioFileLoader(Protocol):
    """Callable signature for loading one audio handle for Cantonese FA."""

    def __call__(self, path: str) -> ASRAudioFile: ...


class _Wave2VecFaRunner(Protocol):
    """Callable signature for running Wave2Vec FA over one audio chunk."""

    def __call__(
        self,
        model: object,
        audio: object,
        words: list[str],
    ) -> list[tuple[str, tuple[int, int]]]: ...


# ---------------------------------------------------------------------------
# Module-level state (populated by load_cantonese_fa)
# ---------------------------------------------------------------------------

_model: Wave2VecFAHandle | None = None
_pc: _PyCantonese | None = None


@dataclass(frozen=True, slots=True)
class CantoneseFaHost:
    """Runtime dependency bundle for Cantonese FA inference.

    Production uses the host built from loaded module state. Tests can provide a
    fake host directly, which keeps the Cantonese FA seam explicit and avoids
    monkeypatching module globals or lazily imported helpers.
    """

    model: object
    romanizer: _PyCantonese
    load_audio_file: _AudioFileLoader
    infer_wave2vec_fa: _Wave2VecFaRunner


# ---------------------------------------------------------------------------
# Jyutping conversion
# ---------------------------------------------------------------------------


def _hanzi_to_jyutping(pc: _PyCantonese, text: str) -> str:
    """Convert Cantonese hanzi token to MMS-alignment jyutping form.

    - Uses ``pycantonese.characters_to_jyutping()`` to obtain per-character
      romanization.
    - Strips tone digits (0--9).
    - Joins multiple character pronunciations with an apostrophe
      (e.g. ``"nei'hou"`` for two characters).
    - Unknown characters (where pycantonese returns ``None``) pass through
      unchanged.

    Returns the original *text* unchanged if conversion produces nothing.
    """
    pairs = pc.characters_to_jyutping(text)
    try:
        jyut = " ".join(
            pron for _char, pron in pairs if isinstance(pron, str) and pron
        )
    except TypeError:
        return text

    if not jyut:
        return text

    # Strip tone digits
    jyut = re.sub(r"[0-9]", "", jyut)
    # Collapse whitespace into apostrophes (one per syllable boundary)
    jyut = re.sub(r"\s+", "'", jyut).strip("'")
    return jyut or text


def _maybe_romanize(pc: _PyCantonese, word: str, lang: LanguageCode) -> str:
    """Romanize *word* only when *lang* is Cantonese (``"yue"``)."""
    if lang == "yue":
        return _hanzi_to_jyutping(pc, word)
    return word


# ---------------------------------------------------------------------------
# Load
# ---------------------------------------------------------------------------


def load_cantonese_fa(
    lang: LanguageCode,
    engine_overrides: EngineOverrides | None,
    *,
    device_policy=None,
) -> None:
    """Load the Wave2Vec FA model and pycantonese converter.

    Called once at worker startup.  Stores the model and converter in
    module-level state for ``infer_cantonese_fa``.

    Parameters
    ----------
    lang : str
        ISO 639-3 language code (e.g. ``"yue"``).
    engine_overrides : EngineOverrides or None
        Engine overrides (currently unused, reserved for future
        model selection).
    """
    global _model, _pc  # noqa: PLW0603

    try:
        import pycantonese as pc
    except ImportError as exc:
        raise ImportError(
            "Cantonese FA dependency 'pycantonese' is missing from this "
            "environment. Reinstall batchalign3 or install pycantonese."
        ) from exc

    from batchalign.inference.fa import load_wave2vec_fa

    _pc = pc
    _model = load_wave2vec_fa(device_policy=device_policy)
    L.info("Cantonese FA model loaded: lang=%s", lang)


def default_cantonese_fa_host() -> CantoneseFaHost | None:
    """Build the production Cantonese FA host from loaded module state."""
    if _model is None or _pc is None:
        return None

    from batchalign.inference.audio import load_audio_file
    from batchalign.inference.fa import infer_wave2vec_fa

    return CantoneseFaHost(
        model=_model,
        romanizer=_pc,
        load_audio_file=load_audio_file,
        infer_wave2vec_fa=infer_wave2vec_fa,
    )


# ---------------------------------------------------------------------------
# Infer
# ---------------------------------------------------------------------------


def infer_cantonese_fa(
    req: BatchInferRequest,
    *,
    host: CantoneseFaHost | None = None,
) -> BatchInferResponse:
    """Process a batch of FA inference items with Cantonese jyutping preprocessing.

    Each item is a :class:`FaInferItem`.  For ``yue`` language, words are
    converted to tone-stripped jyutping before Wave2Vec alignment.  For other
    languages the words pass through unchanged.

    Parameters
    ----------
    req : BatchInferRequest
        Batch of ``FaInferItem`` payloads.

    Returns
    -------
    BatchInferResponse
        One :class:`InferResponse` per item, each containing a
        :class:`Wave2VecIndexedResponse`.
    """
    resolved_host = host or default_cantonese_fa_host()
    if resolved_host is None:
        return BatchInferResponse(
            results=[
                InferResponse(
                    error="Cantonese FA model not loaded — call load_cantonese_fa first",
                    elapsed_s=0.0,
                )
                for _ in req.items
            ]
        )

    model = resolved_host.model
    pc = resolved_host.romanizer
    lang = req.lang

    t0 = time.monotonic()
    audio_cache: dict[str, ASRAudioFile] = {}
    lock = threading.Lock()
    n = len(req.items)
    results: list[InferResponse] = []

    for item_idx, raw_item in enumerate(req.items):
        # --- validate ---
        try:
            item = FaInferItem.model_validate(raw_item)
        except ValidationError:
            results.append(
                InferResponse(error="Invalid FaInferItem", elapsed_s=0.0)
            )
            continue

        # --- empty words shortcut ---
        if not item.words:
            results.append(
                InferResponse(
                    result=Wave2VecIndexedResponse(
                        indexed_timings=[]
                    ).model_dump(),
                    elapsed_s=0.0,
                )
            )
            continue

        try:
            # --- load/cache audio ---
            if item.audio_path not in audio_cache:
                audio_cache[item.audio_path] = resolved_host.load_audio_file(item.audio_path)
            audio_file = audio_cache[item.audio_path]
            audio_chunk = audio_file.chunk(item.audio_start_ms, item.audio_end_ms)

            # --- romanize words for alignment ---
            romanized_words = [
                _maybe_romanize(pc, w, lang) for w in item.words
            ]

            # --- run Wave2Vec FA ---
            with lock:
                wave2vec_results = resolved_host.infer_wave2vec_fa(
                    model,
                    audio_chunk,
                    romanized_words,
                )

            # --- build indexed timings ---
            indexed_timings: list[FaIndexedTiming | None] = [None] * len(
                item.words
            )
            for i, (_, (start, end)) in enumerate(
                wave2vec_results[: len(item.words)]
            ):
                indexed_timings[i] = FaIndexedTiming(
                    start_ms=start, end_ms=end
                )

            results.append(
                InferResponse(
                    result=Wave2VecIndexedResponse(
                        indexed_timings=indexed_timings
                    ).model_dump(),
                    elapsed_s=0.0,
                )
            )

        except Exception as e:
            L.warning(
                "Cantonese FA infer failed for item %d: %s",
                item_idx,
                e,
                exc_info=True,
            )
            results.append(
                InferResponse(
                    result=Wave2VecIndexedResponse(
                        indexed_timings=[]
                    ).model_dump(),
                    elapsed_s=0.0,
                )
            )

    elapsed = time.monotonic() - t0

    # Record total elapsed on the first result
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed
        )

    L.info("batch_infer cantonese_fa: %d items, %.3fs", n, elapsed)
    return BatchInferResponse(results=results)


__all__ = [
    "CantoneseFaHost",
    "_hanzi_to_jyutping",
    "_maybe_romanize",
    "default_cantonese_fa_host",
    "infer_cantonese_fa",
    "load_cantonese_fa",
]
