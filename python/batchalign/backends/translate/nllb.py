"""NllbTranslateBackend: local NLLB-200-distilled-1.3B translation.

Faithful port of tbtbt's `_load_nllb_translate`
(`batchalign/worker/_model_loading/translation.py`). Loads
`facebook/nllb-200-distilled-1.3B` via HuggingFace
`AutoModelForSeq2SeqLM`, evaluates in greedy mode, and decodes with the
target language forced via `forced_bos_token_id`.

Self-hosted, no outbound network at inference. ~5 GB on first download.
The encoder-decoder is small enough for CPU on Apple Silicon; on hosts
with CUDA the operator can opt in via `device="cuda"`. NLLB is the
canonical fallback for source languages outside Tencent TMT's supported
set (Cantonese is the prototypical case).
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import BatchPolicy, Translate

# ISO-639-3 → FLORES-200 language tag. tbtbt parity:
# `batchalign/worker/_model_loading/translation.py::_ISO_639_3_TO_FLORES_200`.
# Closed set — an unmapped source raises rather than silently misclassifying.
_ISO_639_3_TO_FLORES_200: dict[str, str] = {
    "eng": "eng_Latn",
    "spa": "spa_Latn",
    "fra": "fra_Latn",
    "deu": "deu_Latn",
    "ita": "ita_Latn",
    "por": "por_Latn",
    "nld": "nld_Latn",
    "cmn": "zho_Hans",
    "zho": "zho_Hans",
    "yue": "yue_Hant",
    "jpn": "jpn_Jpan",
    "kor": "kor_Hang",
    "rus": "rus_Cyrl",
}

# Same FLORES tags for target — NLLB uses one tag space.
_TARGET_TO_FLORES: dict[str, str] = dict(_ISO_639_3_TO_FLORES_200)


class NllbTranslateBackend(Translate):
    """NLLB-200-distilled-1.3B local translator (tbtbt parity)."""

    def __init__(
        self,
        *,
        target: str = "eng",
        model_id: str = "facebook/nllb-200-distilled-1.3B",
        device: str | None = None,
        max_length: int = 256,
        batch_size: int = 8,
        batch_window_ms: int = 50,
    ) -> None:
        from transformers import (  # type: ignore[import-not-found]
            AutoModelForSeq2SeqLM,
            AutoTokenizer,
        )

        self._target = target
        self._target_flores = _TARGET_TO_FLORES.get(target)
        if self._target_flores is None:
            raise ValueError(
                f"NLLB has no FLORES-200 mapping for target language "
                f"{target!r}; known targets: {sorted(_TARGET_TO_FLORES)}"
            )
        self._model_id = model_id
        self._max_length = max_length

        self._tokenizer = AutoTokenizer.from_pretrained(model_id)
        self._model = AutoModelForSeq2SeqLM.from_pretrained(model_id)
        # Move to device if requested; default CPU keeps memory predictable.
        if device:
            self._model = self._model.to(device)
        self._device = device or "cpu"
        # eval() disables dropout / sets BN to inference mode — without it
        # generation is non-deterministic and ~10% lower-quality (tbtbt).
        if hasattr(self._model, "eval"):
            self._model.eval()
        # Resolve target BOS token once so generate() doesn't repeat the lookup.
        self._target_bos_id = self._tokenizer.convert_tokens_to_ids(self._target_flores)
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # v2: terminator stripped from input (tbtbt parity).
        return f"nllb:{self._model_id}:v2"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import TranslateInput, TranslateOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, TranslateInput):
                raise TypeError(
                    f"NllbTranslateBackend does not handle: {type(item).__name__}"
                )
            src_iso = (
                item.source.value
                if item.source.kind == "code" and item.source.value
                else "eng"
            )
            flores_src = _ISO_639_3_TO_FLORES_200.get(src_iso)
            if flores_src is None:
                raise ValueError(
                    f"NLLB has no FLORES-200 mapping for source language "
                    f"{src_iso!r}; add it to _ISO_639_3_TO_FLORES_200"
                )
            translations: list[str] = []
            for text in item.utterances:
                if not text.strip():
                    translations.append("")
                    continue
                # tbtbt parity: it sends `words.join(" ")` — bare words, no
                # CHAT terminator. BA3's taskrunner currently appends the
                # typed terminator string. Strip the trailing terminator
                # (and trailing whitespace) so NLLB sees the same input.
                stripped = text.rstrip().rstrip(".!?;:").rstrip()
                # tbtbt parity: set src_lang on the tokenizer for each call,
                # tokenize, generate with forced_bos = target lang, decode.
                self._tokenizer.src_lang = flores_src
                inputs = self._tokenizer(stripped, return_tensors="pt")
                if self._device != "cpu":
                    inputs = {k: v.to(self._device) for k, v in inputs.items()}
                generated = self._model.generate(
                    **inputs,
                    forced_bos_token_id=self._target_bos_id,
                    max_length=self._max_length,
                )
                translated = self._tokenizer.decode(
                    generated[0], skip_special_tokens=True
                )
                translations.append(str(translated))
            outputs.append(
                TranslateOutput(source_id=item.source_id, utterances=translations)
            )
        return outputs


__all__ = ["NllbTranslateBackend"]
