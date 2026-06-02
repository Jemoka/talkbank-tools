"""TencentTmtBackend: Tencent Cloud TMT (Text Translation) translation.

Faithful port of tbtbt's `_load_tencent_translate`
(`batchalign/worker/_model_loading/translation.py`). Calls
`tmt.v20180321.TmtClient.TextTranslate` with `ProjectId=0`. Credentials
share the `[asr] engine.tencent.*` section with the Tencent ASR backend
(SecretId / SecretKey / region); the CAM principal must have
`tmt:TextTranslate` permission and TMT must be "opened" on the account.

Free tier: 5 M characters / month, 5 QPS for `TextTranslate`. We
self-throttle at 5 QPS (0.2 s/req) to avoid `RequestLimitExceeded`.

Cantonese (`yue`) is **not** a Tencent TMT source language; that route
must use [[NllbTranslateBackend]] or [[AliyunTranslateBackend]] instead
(both handle yue first-class).
"""

from __future__ import annotations

import time
from typing import Any

from batchalign import config
from batchalign.backends.base import BatchPolicy, Translate

# ISO-639-3 → Tencent TMT source-language code. tbtbt parity:
# `batchalign/worker/_model_loading/translation.py::_ISO_639_3_TO_TENCENT_LANG`.
# Closed set — an unmapped source raises rather than silently
# misclassifying. Notable quirk: Tencent uses `kor` (3-letter) for
# Korean, not `ko`.
_ISO_639_3_TO_TENCENT_LANG: dict[str, str] = {
    "eng": "en",
    "spa": "es",
    "fra": "fr",
    "deu": "de",
    "ita": "it",
    "por": "pt",
    "rus": "ru",
    "cmn": "zh",
    "zho": "zh",
    "jpn": "ja",
    "kor": "kor",
    "ara": "ar",
    "tha": "th",
    "vie": "vi",
    "tur": "tr",
    "ind": "id",
    "msa": "ms",
}

# Target language must also be in Tencent's supported set; English is the
# only target tbtbt ever requests, but expose the mapping for completeness.
_ISO_639_3_TO_TENCENT_TARGET: dict[str, str] = dict(_ISO_639_3_TO_TENCENT_LANG)

# 5 QPS limit on the free tier ⇒ 0.2 s per request guarantees ≤5 QPS.
_TENCENT_TMT_RATE_S = 0.2


class TencentTmtBackend(Translate):
    """Tencent Cloud TMT text translation (tbtbt parity)."""

    def __init__(
        self,
        *,
        target: str = "eng",
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from tencentcloud.common.credential import Credential  # type: ignore[import-not-found]
        from tencentcloud.common.profile.client_profile import ClientProfile  # type: ignore[import-not-found]
        from tencentcloud.common.profile.http_profile import HttpProfile  # type: ignore[import-not-found]
        from tencentcloud.tmt.v20180321.tmt_client import TmtClient  # type: ignore[import-not-found]

        creds = config.get_provider("tencent", interactive=True)
        try:
            secret_id = creds["id"]
            secret_key = creds["key"]
            region = creds["region"]
        except KeyError as e:
            raise RuntimeError(
                "Tencent Cloud not configured: set engine.tencent.{id,key,region} "
                f"under [asr] in ~/.batchalign.ini (missing {e})."
            ) from e

        target_code = _ISO_639_3_TO_TENCENT_TARGET.get(target)
        if target_code is None:
            raise ValueError(
                f"Tencent TMT has no target-language mapping for "
                f"{target!r}; known targets: {sorted(_ISO_639_3_TO_TENCENT_TARGET)}"
            )
        self._target = target
        self._target_code = target_code

        cred = Credential(secret_id, secret_key)
        http_profile = HttpProfile()
        http_profile.endpoint = "tmt.tencentcloudapi.com"
        client_profile = ClientProfile()
        client_profile.httpProfile = http_profile
        self._client = TmtClient(cred, region, client_profile)
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"tencent-tmt:{self._target}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from tencentcloud.common.exception.tencent_cloud_sdk_exception import (  # type: ignore[import-not-found]
            TencentCloudSDKException,
        )
        from tencentcloud.tmt.v20180321 import models  # type: ignore[import-not-found]
        from batchalign._core.proto import TranslateInput, TranslateOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, TranslateInput):
                raise TypeError(
                    f"TencentTmtBackend does not handle: {type(item).__name__}"
                )
            src_iso = (
                item.source.value
                if item.source.kind == "code" and item.source.value
                else "eng"
            )
            src_code = _ISO_639_3_TO_TENCENT_LANG.get(src_iso)
            if src_code is None:
                raise ValueError(
                    f"Tencent TMT does not support source language {src_iso!r}; "
                    f"use --engine nllb (self-hosted, handles every language) "
                    f"or --engine aliyun (cloud, supports Cantonese) instead."
                )
            translations: list[str] = []
            for text in item.utterances:
                if not text:
                    translations.append("")
                    continue
                # tbtbt parity: it sends `words.join(" ")` — bare words, no
                # CHAT terminator. BA3's taskrunner appends the typed
                # terminator; strip it before sending.
                stripped = text.rstrip().rstrip(".!?;:").rstrip()
                if not stripped:
                    translations.append("")
                    continue
                req = models.TextTranslateRequest()
                req.SourceText = stripped
                req.Source = src_code
                req.Target = self._target_code
                req.ProjectId = 0
                try:
                    resp = self._client.TextTranslate(req)
                except TencentCloudSDKException as exc:
                    raise RuntimeError(
                        f"Tencent TMT translation failed: {exc}"
                    ) from exc
                translations.append(str(resp.TargetText))
                # Self-throttle to free-tier QPS (5 req/s) — tbtbt parity.
                time.sleep(_TENCENT_TMT_RATE_S)
            outputs.append(
                TranslateOutput(source_id=item.source_id, utterances=translations)
            )
        return outputs


__all__ = ["TencentTmtBackend"]
