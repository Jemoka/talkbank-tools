"""Aliyun NLS file-transcription ASR backend.

Reads ``engine.aliyun.{ak_id,ak_secret,ak_appkey}`` from ``~/.batchalign.ini``.
Requires the ``alibabacloud_nls_filetrans20180817`` SDK at runtime;
imports are lazy so the backend can be constructed without it.
"""

from __future__ import annotations

import json
import time
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy
from batchalign import config


class AliyunAsrBackend(ASR):
    """Aliyun NLS file-transcription service."""

    _PROVIDER = "aliyun"

    def __init__(self, *, poll_interval_s: float = 5.0, timeout_s: float = 3600.0) -> None:
        creds = config.get_provider(self._PROVIDER)
        self._ak_id = creds.get("ak_id", "")
        self._ak_secret = creds.get("ak_secret", "")
        self._app_key = creds.get("ak_appkey", "")
        self._poll = poll_interval_s
        self._timeout = timeout_s
        self._policy = BatchPolicy.one()

    @property
    def name(self) -> str:
        return f"aliyun:nls:{self._app_key}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from aliyunsdkcore.client import AcsClient  # type: ignore[import-not-found]
        from aliyunsdkcore.request import CommonRequest  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrInput, AsrOutput

        if not (self._ak_id and self._ak_secret and self._app_key):
            raise RuntimeError(
                "AliyunAsrBackend missing credentials; set engine.aliyun.{ak_id,ak_secret,ak_appkey}."
            )
        client = AcsClient(self._ak_id, self._ak_secret, "cn-shanghai")
        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"AliyunAsrBackend does not handle: {type(item).__name__}"
                )
            url = self._upload_to_oss(item)
            task_id = self._submit(client, CommonRequest, url)
            result = self._poll_task(client, CommonRequest, task_id)
            outputs.append(_asr_from_aliyun(result, item.source_id))
        return outputs

    def _upload_to_oss(self, item: Any) -> str:
        """Upload audio to OSS, return the public URL.

        OSS upload is out of scope for the open-source repo. Subclass and
        override this method, or pass ``AsrInput`` whose ``audio.pcm_f32le``
        is already an HTTPS URL encoded in bytes.
        """
        raise NotImplementedError(
            "Aliyun NLS file-transcription requires the audio to be reachable "
            "via an HTTPS URL. Override `_upload_to_oss` to push audio to OSS, "
            "or use FunAsrBackend (local) for inline-audio workflows."
        )

    def _submit(self, client: Any, CommonRequest: Any, url: str) -> str:
        req = CommonRequest()
        req.set_domain("filetrans.cn-shanghai.aliyuncs.com")
        req.set_version("2018-08-17")
        req.set_action_name("SubmitTask")
        req.set_method("POST")
        body = {"appkey": self._app_key, "file_link": url, "version": "4.0", "enable_words": True}
        req.add_body_params("Task", json.dumps(body))
        resp = json.loads(client.do_action_with_exception(req))
        return resp["TaskId"]

    def _poll_task(self, client: Any, CommonRequest: Any, task_id: str) -> dict[str, Any]:
        deadline = time.monotonic() + self._timeout
        while True:
            req = CommonRequest()
            req.set_domain("filetrans.cn-shanghai.aliyuncs.com")
            req.set_version("2018-08-17")
            req.set_action_name("GetTaskResult")
            req.set_method("GET")
            req.add_query_param("TaskId", task_id)
            resp = json.loads(client.do_action_with_exception(req))
            status = resp.get("StatusText")
            if status == "SUCCESS":
                return resp
            if status in ("RUNNING", "QUEUEING"):
                if time.monotonic() > deadline:
                    raise TimeoutError(f"Aliyun ASR task {task_id} timed out")
                time.sleep(self._poll)
                continue
            raise RuntimeError(f"Aliyun ASR task {task_id} failed: {resp!r}")


def _asr_from_aliyun(data: dict[str, Any], source_id: str) -> Any:
    """Project an Aliyun NLS result into :class:`AsrOutput`."""
    from batchalign._core.proto import AsrOutput, AsrSegment, AsrWord

    sentences = (data.get("Result") or {}).get("Sentences") or []
    segments: list[Any] = []
    for sent in sentences:
        words = []
        for w in sent.get("Words", []):
            words.append(
                AsrWord(
                    text=w.get("Word", ""),
                    start_ms=int(w.get("BeginTime") or 0),
                    end_ms=int(w.get("EndTime") or 0),
                    confidence=None,
                )
            )
        segments.append(
            AsrSegment(
                start_ms=int(sent.get("BeginTime") or 0),
                end_ms=int(sent.get("EndTime") or 0),
                text=sent.get("Text", ""),
                speaker=str(sent.get("SpeakerId")) if sent.get("SpeakerId") is not None else None,
                words=words,
            )
        )
    return AsrOutput(source_id=source_id, segments=segments)


__all__ = ["AliyunAsrBackend"]
