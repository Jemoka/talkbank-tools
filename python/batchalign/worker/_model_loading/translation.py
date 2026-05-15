"""Translation-engine bootstrap helpers for worker startup."""

from __future__ import annotations

import logging

from batchalign.inference._domain_types import LanguageCode, TranslationBackend
from batchalign.worker._types import _state

L = logging.getLogger("batchalign.worker")


def load_translation_engine(engine_overrides: dict[str, str] | None = None) -> None:
    """Load the translation engine for this worker."""
    del engine_overrides
    _state.translate_backend = TranslationBackend.GOOGLE

    try:
        from googletrans import Translator

        async def _do_translate(translator: Translator, text: str) -> str:
            result = await translator.translate(text)
            return str(getattr(result, "text", result))

        def translate_fn(text: str, src_lang: LanguageCode) -> str:
            """Run the async translator behind the worker's synchronous IPC seam."""
            import asyncio

            translator = Translator()
            loop = asyncio.new_event_loop()
            try:
                return loop.run_until_complete(_do_translate(translator, text))
            finally:
                loop.close()

        _state.translate_fn = translate_fn
    except ImportError:
        L.warning("googletrans not available, trying seamless")
        try:
            from transformers import AutoProcessor, SeamlessM4TModel

            from batchalign.worker._progress import (
                HF_ARTIFACTS_SEAMLESS,
                emit_hf_download_if_missing,
            )

            emit_hf_download_if_missing(
                "facebook/hf-seamless-m4t-medium",
                kind="translation",
                artifacts=HF_ARTIFACTS_SEAMLESS,
            )

            processor = AutoProcessor.from_pretrained(  # type: ignore[no-untyped-call]
                "facebook/hf-seamless-m4t-medium"
            )
            model = SeamlessM4TModel.from_pretrained("facebook/hf-seamless-m4t-medium")
            if hasattr(model, "eval"):
                model.eval()  # type: ignore[no-untyped-call]

            def seamless_fn(text: str, src_lang: LanguageCode) -> str:
                """Translate one text payload through SeamlessM4T."""
                inputs = processor(text=text, src_lang=src_lang, return_tensors="pt")
                output = model.generate(**inputs, tgt_lang="eng", generate_speech=False)
                return str(processor.decode(output[0].tolist()[0], skip_special_tokens=True))

            _state.translate_backend = TranslationBackend.SEAMLESS
            _state.translate_fn = seamless_fn
        except ImportError:
            L.error("No translation engine available")
