"""AliyunTranslateBackend: Aliyun (Alibaba Cloud) MT General translation.

Faithful port of tbtbt's `_load_aliyun_translate`
(`batchalign/worker/_model_loading/translation.py`). Calls
`aliyunsdkalimt.TranslateGeneralRequest` via `AcsClient`. Credentials
come from the same `[asr] engine.aliyun.*` section that the Aliyun ASR
backend uses (`ak_id` / `ak_secret`); MT does **not** need the
`ak_appkey` that ASR uses.

The canonical cloud option for Cantonese (`yue`) source — Tencent TMT
does not list `yue` as a source language, Aliyun does. Region is pinned
to `cn-hangzhou` because Aliyun MT serves from a single global endpoint
(`mt.aliyuncs.com`); region only affects request signing.
"""

from __future__ import annotations

import json
from typing import Any

from batchalign import config
from batchalign.backends.base import BatchPolicy, Translate

# ISO-639-3 → Aliyun MT source-language code. tbtbt parity:
# `batchalign/worker/_model_loading/translation.py::_ISO_639_3_TO_ALIYUN_LANG`.
# Notable quirks: Aliyun uses `spa`/`fra` (3-letter) for Spanish/French,
# `vie` for Vietnamese, and exposes `yue` as a first-class source code
# (the reason this backend exists alongside Tencent TMT).
_ISO_639_3_TO_ALIYUN_LANG: dict[str, str] = {
    "eng": "en",
    "spa": "spa",
    "fra": "fra",
    "deu": "de",
    "ita": "it",
    "por": "pt",
    "rus": "ru",
    "cmn": "zh",
    "zho": "zh",
    "yue": "yue",
    "jpn": "ja",
    "kor": "ko",
    "ara": "ar",
    "tha": "th",
    "vie": "vie",
    "tur": "tr",
    "ind": "id",
    "msa": "ms",
}

_ISO_639_3_TO_ALIYUN_TARGET: dict[str, str] = dict(_ISO_639_3_TO_ALIYUN_LANG)

_ALIYUN_MT_REGION = "cn-hangzhou"
_ALIYUN_MT_FORMAT_TYPE = "text"
_ALIYUN_MT_SCENE = "general"


class AliyunTranslateBackend(Translate):
    """Aliyun MT General translation (tbtbt parity)."""

    def __init__(
        self,
        *,
        target: str = "eng",
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from aliyunsdkcore.client import AcsClient  # type: ignore[import-not-found]

        creds = config.get_provider("aliyun")
        try:
            ak_id = creds["ak_id"]
            ak_secret = creds["ak_secret"]
        except KeyError as e:
            raise RuntimeError(
                "Aliyun not configured: set engine.aliyun.{ak_id,ak_secret} "
                f"under [asr] in ~/.batchalign.ini (missing {e})."
            ) from e

        target_code = _ISO_639_3_TO_ALIYUN_TARGET.get(target)
        if target_code is None:
            raise ValueError(
                f"Aliyun MT has no target-language mapping for {target!r}; "
                f"known targets: {sorted(_ISO_639_3_TO_ALIYUN_TARGET)}"
            )
        self._target = target
        self._target_code = target_code

        self._client = AcsClient(ak_id, ak_secret, _ALIYUN_MT_REGION)
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"aliyun-mt:{self._target}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from aliyunsdkalimt.request.v20181012.TranslateGeneralRequest import (  # type: ignore[import-not-found]
            TranslateGeneralRequest,
        )
        from aliyunsdkcore.acs_exception.exceptions import (  # type: ignore[import-not-found]
            ClientException,
            ServerException,
        )
        from batchalign._core.proto import TranslateInput, TranslateOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, TranslateInput):
                raise TypeError(
                    f"AliyunTranslateBackend does not handle: {type(item).__name__}"
                )
            src_iso = (
                item.source.value
                if item.source.kind == "code" and item.source.value
                else "eng"
            )
            src_code = _ISO_639_3_TO_ALIYUN_LANG.get(src_iso)
            if src_code is None:
                raise ValueError(
                    f"Aliyun MT has no mapped source language for {src_iso!r}; "
                    f"use --engine nllb for this language"
                )
            translations: list[str] = []
            for text in item.utterances:
                if not text:
                    translations.append("")
                    continue
                # tbtbt parity: bare `words.join(" ")` — no CHAT terminator.
                stripped = text.rstrip().rstrip(".!?;:").rstrip()
                if not stripped:
                    translations.append("")
                    continue
                req = TranslateGeneralRequest()
                req.set_FormatType(_ALIYUN_MT_FORMAT_TYPE)
                req.set_SourceLanguage(src_code)
                req.set_TargetLanguage(self._target_code)
                req.set_SourceText(stripped)
                req.set_Scene(_ALIYUN_MT_SCENE)
                try:
                    raw = self._client.do_action_with_exception(req)
                except (ClientException, ServerException) as exc:
                    raise RuntimeError(
                        f"Aliyun MT translation failed: {exc}"
                    ) from exc
                try:
                    parsed = json.loads(raw)
                except json.JSONDecodeError as exc:
                    raise RuntimeError(
                        f"Aliyun MT returned non-JSON response: {raw!r}"
                    ) from exc
                # Response envelope: {"Code": "200", "Data": {"Translated": ...,
                # "WordCount": ..., "DetectedLanguage": ...}, "RequestId": ...}.
                # `do_action_with_exception` already raised on non-200 codes.
                data = parsed.get("Data") or {}
                translated = data.get("Translated")
                if not isinstance(translated, str):
                    raise RuntimeError(
                        f"Aliyun MT response missing Data.Translated string: {parsed!r}"
                    )
                translations.append(translated)
            outputs.append(
                TranslateOutput(source_id=item.source_id, utterances=translations)
            )
        return outputs


__all__ = ["AliyunTranslateBackend"]
