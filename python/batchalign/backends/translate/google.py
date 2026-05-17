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

from typing import Any

from batchalign.backends.base import Translate, BatchPolicy
from batchalign import config


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
        # googletrans fallback: one call per utterance.
        out = []
        for text in texts:
            kwargs = {"dest": target}
            if source:
                kwargs["src"] = source
            out.append(self._client.translate(text, **kwargs).text)
        return out


__all__ = ["GoogleTranslateBackend"]
