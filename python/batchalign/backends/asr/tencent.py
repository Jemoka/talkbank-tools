"""Tencent Cloud ASR backend (async recognition over HTTPS).

Reads ``engine.tencent.{id,key,region,bucket}`` from ``~/.batchalign.ini``.
Uses TC3-HMAC-SHA256 signing; see
https://cloud.tencent.com/document/api/1093/35640.
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import json
import time
import urllib.request
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy
from batchalign import config


class TencentAsrBackend(ASR):
    """Tencent Cloud ASR — async recognition over HTTPS.

    Tencent's recognition endpoint accepts a base64-encoded audio blob
    (<=5MB) or a COS URL. We use the inline-blob mode for simplicity.
    """

    _PROVIDER = "tencent"

    def __init__(
        self,
        *,
        engine_model_type: str = "16k_zh-PY",
        poll_interval_s: float = 5.0,
        timeout_s: float = 3600.0,
    ) -> None:
        creds = config.get_provider(self._PROVIDER)
        self._secret_id = creds.get("id", "")
        self._secret_key = creds.get("key", "")
        self._region = creds.get("region", "ap-guangzhou")
        self._bucket = creds.get("bucket", "")
        self._engine = engine_model_type
        self._poll = poll_interval_s
        self._timeout = timeout_s
        self._policy = BatchPolicy.one()

    @property
    def name(self) -> str:
        return f"tencent:{self._engine}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput
        from batchalign.backends.asr.vllm import pcm_to_wav_bytes

        if not (self._secret_id and self._secret_key):
            raise RuntimeError(
                "TencentAsrBackend missing credentials. Set "
                "engine.tencent.id and engine.tencent.key in ~/.batchalign.ini."
            )
        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"TencentAsrBackend does not handle: {type(item).__name__}"
                )
            wav = pcm_to_wav_bytes(item.audio)
            task_id = self._create_task(wav)
            result = self._poll_task(task_id)
            outputs.append(_asr_from_tencent(result, item.source_id))
        return outputs

    def _create_task(self, wav_bytes: bytes) -> int:
        params = {
            "EngineModelType": self._engine,
            "ChannelNum": 1,
            "ResTextFormat": 2,
            "SourceType": 1,
            "Data": base64.b64encode(wav_bytes).decode("ascii"),
            "DataLen": len(wav_bytes),
        }
        resp = self._tencent_post("CreateRecTask", params)
        return int(resp["Data"]["TaskId"])

    def _poll_task(self, task_id: int) -> dict[str, Any]:
        deadline = time.monotonic() + self._timeout
        while True:
            resp = self._tencent_post("DescribeTaskStatus", {"TaskId": task_id})
            data = resp["Data"]
            status = data.get("StatusStr") or data.get("Status")
            if status in ("success", 2):
                return data
            if status in ("failed", 3):
                raise RuntimeError(f"Tencent ASR task {task_id} failed: {data!r}")
            if time.monotonic() > deadline:
                raise TimeoutError(f"Tencent ASR task {task_id} timed out")
            time.sleep(self._poll)

    def _tencent_post(self, action: str, params: dict[str, Any]) -> dict[str, Any]:
        host = "asr.tencentcloudapi.com"
        service = "asr"
        ts = int(time.time())
        date = time.strftime("%Y-%m-%d", time.gmtime(ts))

        payload = json.dumps(params, separators=(",", ":"))
        canonical_request = (
            "POST\n/\n\n"
            "content-type:application/json; charset=utf-8\n"
            f"host:{host}\n\n"
            "content-type;host\n"
            + hashlib.sha256(payload.encode("utf-8")).hexdigest()
        )
        credential_scope = f"{date}/{service}/tc3_request"
        string_to_sign = (
            "TC3-HMAC-SHA256\n"
            f"{ts}\n"
            f"{credential_scope}\n"
            + hashlib.sha256(canonical_request.encode("utf-8")).hexdigest()
        )

        def _hmac(key: bytes, msg: str) -> bytes:
            return hmac.new(key, msg.encode("utf-8"), hashlib.sha256).digest()

        secret_date = _hmac(("TC3" + self._secret_key).encode("utf-8"), date)
        secret_service = _hmac(secret_date, service)
        secret_signing = _hmac(secret_service, "tc3_request")
        signature = hmac.new(
            secret_signing, string_to_sign.encode("utf-8"), hashlib.sha256
        ).hexdigest()

        authorization = (
            f"TC3-HMAC-SHA256 Credential={self._secret_id}/{credential_scope}, "
            f"SignedHeaders=content-type;host, Signature={signature}"
        )
        req = urllib.request.Request(
            f"https://{host}/",
            data=payload.encode("utf-8"),
            method="POST",
            headers={
                "Authorization": authorization,
                "Content-Type": "application/json; charset=utf-8",
                "Host": host,
                "X-TC-Action": action,
                "X-TC-Timestamp": str(ts),
                "X-TC-Version": "2019-06-14",
                "X-TC-Region": self._region,
            },
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = json.loads(resp.read().decode("utf-8"))
        if "Response" not in body or body["Response"].get("Error"):
            raise RuntimeError(f"Tencent ASR {action} failed: {body!r}")
        return body["Response"]


def _asr_from_tencent(data: dict[str, Any], source_id: str) -> Any:
    """Project a Tencent ASR success payload into :class:`AsrOutput`."""
    from batchalign._core.proto import AsrOutput, AsrSegment, AsrWord

    words: list[Any] = []
    for det in data.get("ResultDetail", []):
        for w in det.get("Words", []):
            start_ms = int(w.get("OffsetStartMs") or 0)
            end_ms = int(w.get("OffsetEndMs") or 0)
            words.append(
                AsrWord(
                    text=w.get("Word", ""),
                    start_ms=start_ms,
                    end_ms=end_ms,
                    confidence=None,
                )
            )
    if not words:
        return AsrOutput(source_id=source_id, segments=[])
    text = data.get("Result", "") or " ".join(w.text for w in words)
    seg = AsrSegment(
        start_ms=words[0].start_ms,
        end_ms=words[-1].end_ms,
        text=text,
        speaker=None,
        words=words,
    )
    return AsrOutput(source_id=source_id, segments=[seg])


__all__ = ["TencentAsrBackend"]
