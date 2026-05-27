"""GoogleTranslateBackend: translation via Google Cloud Translate.

We use ``google-cloud-translate`` (v2 API) — the official client. Auth
is taken from either:

* an explicit ``api_key=`` constructor argument,
* ``BATCHALIGN_GOOGLE_KEY`` / ``[translate] engine.google.key`` in
  ``~/.batchalign.ini`` (a v2 API key string), or
* default application credentials (``GOOGLE_APPLICATION_CREDENTIALS``
  env var pointing at a service-account JSON) when no key is given.

If the official client is unavailable we fall back to the ``googletrans``
free-tier wrapper that BA2 originally used — handy for research
workloads but rate-limited.
"""

from __future__ import annotations

import asyncio
import threading
from concurrent.futures import ThreadPoolExecutor
from typing import Any

from batchalign.backends.base import Translate, BatchPolicy
from batchalign import config

# Utterance / morphology punctuation BA2 inserts a leading space before in the
# translated text (`apple.` → `apple .`). Mirrors batchalign2/constants.py.
_ENDING_PUNCT = [".", "?", "!", "+//.", "+/.", "+...", '+"/.', "+..?", '+".', "+//?", "+.", "+!?", "+/?", "...", "(.)"]
_MOR_PUNCT = ["‡", "„", ","]
_PUNCT_SPACING = _MOR_PUNCT + _ENDING_PUNCT

# ISO-639-3 → -1 for the few CJK source languages BA2 special-cases.
_CJK_SOURCES = {"yue", "zho", "zh", "zh-hans", "zh-hant", "cmn"}


def _iso2(code: str) -> str:
    """Map an ISO-639-3 code to the 2-letter code googletrans expects.

    Passes through codes that are already 2-letter (or compound like
    `zh-cn`). Used only for non-English targets; English uses googletrans'
    default (no `dest`), exactly as BA2 does.
    """
    c = code.strip().lower()
    if len(c) <= 2 or "-" in c:
        return c
    try:
        import pycountry  # type: ignore[import-not-found]

        lang = pycountry.languages.get(alpha_3=c)
        if lang is not None and getattr(lang, "alpha_2", None):
            return lang.alpha_2
    except Exception:
        pass
    return c


def _run_coroutine_sync(coro: Any, timeout: float = 30) -> Any:
    """Run an async coroutine to completion from sync code (BA2's helper).

    Handles being called both outside and inside a running event loop (a
    worker thread): falls back to a fresh loop in a thread pool when needed.
    """
    def run_in_new_loop():
        new_loop = asyncio.new_event_loop()
        asyncio.set_event_loop(new_loop)
        try:
            return new_loop.run_until_complete(coro)
        finally:
            new_loop.close()

    try:
        loop = asyncio.get_running_loop()
    except RuntimeError:
        return asyncio.run(coro)

    if threading.current_thread() is threading.main_thread() and not loop.is_running():
        return loop.run_until_complete(coro)
    with ThreadPoolExecutor() as pool:
        return pool.submit(run_in_new_loop).result(timeout=timeout)


class GoogleTranslateBackend(Translate):
    """Google Cloud Translate client (v2 API) with a googletrans fallback."""

    def __init__(
        self,
        *,
        target: str = "eng",
        api_key: str | None = None,
        batch_size: int = 16,
        batch_window_ms: int = 50,
        force_free: bool = False,
    ) -> None:
        # Target language pin. The runner ships a default `"eng"` on the
        # input; we honour our own constructor arg over it so callers can
        # do `GoogleTranslateBackend(target="zho")` without touching the
        # task wiring.
        self._target = target
        key = api_key if api_key is not None else config.get_api_key("google_translate")
        self._client: Any = None
        self._mode: str
        if force_free:
            self._client = self._make_free_client()
            self._mode = "googletrans:free"
        else:
            try:
                from google.cloud import translate_v2 as g_translate  # type: ignore[import-not-found]

                if key:
                    self._client = g_translate.Client(client_options={"api_key": key})
                else:
                    self._client = g_translate.Client()
                self._mode = "google-cloud-translate:v2"
            except ImportError:
                self._client = self._make_free_client()
                self._mode = "googletrans:free"
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @staticmethod
    def _make_free_client() -> Any:
        from googletrans import Translator  # type: ignore[import-not-found]

        return Translator()

    @property
    def name(self) -> str:
        return self._mode

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import TranslateInput, TranslateOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, TranslateInput):
                raise TypeError(
                    f"GoogleTranslateBackend does not handle: {type(item).__name__}"
                )
            translations = self._translate_many(
                item.utterances,
                source=item.source.value if item.source.kind == "code" else None,
                target=self._target,
            )
            outputs.append(
                TranslateOutput(source_id=item.source_id, utterances=translations)
            )
        return outputs

    # ----- helpers -------------------------------------------------------

    def _translate_many(
        self,
        texts: list[str],
        *,
        source: str | None,
        target: str,
    ) -> list[str]:
        if not texts:
            return []
        if self._mode.startswith("google-cloud-translate"):
            # The v2 client accepts a list and returns a parallel list.
            kwargs: dict[str, Any] = {"target_language": target}
            if source:
                kwargs["source_language"] = source
            result = self._client.translate(texts, **kwargs)
            if isinstance(result, list):
                return [r.get("translatedText", "") for r in result]
            return [result.get("translatedText", "")]
        # googletrans fallback: one call per utterance, faithful to BA2's
        # GoogleTranslateEngine (auto-detect source, default English target,
        # CJK space/period handling, quote/zero-width cleanup, and a leading
        # space before sentence-final punctuation).
        cjk = bool(source and source.lower() in _CJK_SOURCES)
        # BA2 never passes `dest` (googletrans defaults to English); honor a
        # non-English target by mapping to the 2-letter code googletrans wants.
        dest = None if target.lower() in ("eng", "en", "") else _iso2(target)

        # BA2 builds a fresh Translator inside the coroutine for every call —
        # googletrans' httpx client binds to the event loop it was created in,
        # so reusing one client across our per-call loops raises "Event loop
        # is closed". Mirror that: one Translator per utterance.
        from googletrans import Translator  # type: ignore[import-not-found]

        async def _translate(t: str) -> Any:
            translator = Translator()
            if dest:
                return await translator.translate(t, dest=dest)
            return await translator.translate(t)

        out = []
        for text in texts:
            src_text = text
            if cjk:
                src_text = src_text.replace(" ", "").replace(".", "。")
            translated = _run_coroutine_sync(_translate(src_text)).text
            translated = (
                translated.replace("。", ".")
                .replace("’", "'")
                .replace("\t", " ")
                .replace("​", "")
            )
            for j in _PUNCT_SPACING:
                translated = translated.replace(j, " " + j)
            out.append(translated)
        return out


__all__ = ["GoogleTranslateBackend"]
