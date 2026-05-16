"""Translation via a vLLM-served chat model with an OpenAI-compatible API."""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import Translate, BatchPolicy


class VllmTranslateBackend(Translate):
    """Translation via a vLLM-served chat model with an OpenAI-compatible API."""

    def __init__(
        self,
        *,
        model: str = "Qwen/Qwen2.5-7B-Instruct",
        base_url: str = "http://localhost:8000/v1",
        api_key: str = "EMPTY",
        batch_size: int = 16,
        batch_window_ms: int = 50,
    ) -> None:
        from openai import OpenAI  # type: ignore[import-not-found]

        self._client = OpenAI(base_url=base_url, api_key=api_key)
        self._model = model
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"vllm-translate:{self._model}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import TranslateInput, TranslateOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, TranslateInput):
                raise TypeError(
                    f"VllmTranslateBackend does not handle: {type(item).__name__}"
                )
            src = item.source.value if item.source.kind == "code" else "the source language"
            translations: list[str] = []
            for text in item.utterances:
                resp = self._client.chat.completions.create(
                    model=self._model,
                    messages=[
                        {
                            "role": "system",
                            "content": (
                                f"Translate the user's text from {src} to {item.target}. "
                                "Reply with only the translation."
                            ),
                        },
                        {"role": "user", "content": text},
                    ],
                    temperature=0.0,
                )
                translations.append(resp.choices[0].message.content or "")
            outputs.append(
                TranslateOutput(source_id=item.source_id, utterances=translations)
            )
        return outputs


__all__ = ["VllmTranslateBackend"]
