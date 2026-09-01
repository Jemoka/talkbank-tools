"""pyannoteAI cloud speaker diarization backend.

Encodes Batchalign's prepared PCM as a compact temporary MP3, submits a
diarization job to ``api.pyannote.ai``, polls it to completion, and projects the
returned speaker turns into Batchalign's typed protocol. Uploaded media is
temporary; pyannoteAI removes media and job output on its documented retention
schedule.

API key resolution follows the shared Batchalign config system:

1. ``BATCHALIGN_PYANNOTE_KEY``
2. ``~/.batchalign.ini`` ``[diarize] engine.pyannote.key``
"""

from __future__ import annotations

import json
import logging
import time
import urllib.error
import urllib.request
import uuid
from collections.abc import Callable
from typing import Any

from batchalign import config
from batchalign.backends.base import BatchPolicy, Speaker

_LOG = logging.getLogger("batchalign.backends.speaker.pyannote_ai")
_TERMINAL_STATUSES = {"succeeded", "failed", "canceled"}
_CLOUD_MP3_BITRATE_KBPS = 16


class PyannoteAIBackend(Speaker):
    """Cloud diarization through the pyannoteAI REST API."""

    def __init__(
        self,
        api_key: str | None = None,
        *,
        model: str = "precision-2",
        num_speakers: int = 0,
        poll_interval_s: float = 5.0,
        timeout_s: float = 3600.0,
        http_timeout_s: float = 60.0,
        batch_size: int = 8,
        batch_window_ms: int = 250,
        base_url: str = "https://api.pyannote.ai",
        urlopen: Callable[..., Any] | None = None,
        sleep: Callable[[float], None] = time.sleep,
        media_renderer: Callable[[Any], bytes] | None = None,
    ) -> None:
        if model not in {"precision-2", "community-1"}:
            raise ValueError(
                "pyannoteAI model must be 'precision-2' or 'community-1'"
            )
        if num_speakers < 0:
            raise ValueError("num_speakers must be non-negative")
        if poll_interval_s <= 0:
            raise ValueError("poll_interval_s must be positive")
        if timeout_s <= 0 or http_timeout_s <= 0:
            raise ValueError("timeouts must be positive")

        self._api_key = (
            api_key
            if api_key is not None
            else config.get_api_key("pyannote", interactive=True)
        )
        self._model = model
        self._num_speakers = num_speakers
        self._poll_interval = poll_interval_s
        self._timeout = timeout_s
        self._http_timeout = http_timeout_s
        self._base_url = base_url.rstrip("/")
        self._urlopen = urlopen or urllib.request.urlopen
        self._sleep = sleep
        self._media_renderer = media_renderer or _render_mp3
        self._policy = BatchPolicy(
            max_size=batch_size, window_ms=batch_window_ms
        )

    @property
    def name(self) -> str:
        speakers = self._num_speakers or "auto"
        return f"pyannote-ai:{self._model}:speakers-{speakers}:v3"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(
        self, batch: list[Any], *, progress: Any = None, **_kwargs: Any
    ) -> list[Any]:
        from batchalign._core.proto import (
            Diarization,
            DiarizationSegment,
            SpeakerInput,
            SpeakerOutput,
        )

        if any(isinstance(item, SpeakerInput) for item in batch) and not self._api_key:
            raise RuntimeError(
                "pyannoteAI backend has no API key configured. Set "
                "BATCHALIGN_PYANNOTE_KEY or add `[diarize] "
                "engine.pyannote.key = ...` to ~/.batchalign.ini."
            )

        # Upload and submit every unique audio item before polling. This keeps
        # all cloud jobs in flight together, like the Rev atomic-call backend.
        speaker_items: dict[str, Any] = {}
        for item in batch:
            if isinstance(item, SpeakerInput):
                speaker_items.setdefault(str(item.source_id), item)
            else:
                raise TypeError(
                    "PyannoteAIBackend does not handle input type: "
                    f"{type(item).__name__}"
                )

        job_ids: dict[str, str] = {}
        for source_id, item in speaker_items.items():
            media_url = self._upload_audio(item.audio)
            speaker_count = int(item.num_speakers or self._num_speakers or 0)
            job_ids[source_id] = self._submit_job(media_url, speaker_count)
        completed = self._poll_until_all_done(job_ids)

        outputs: list[Any] = []
        for item in batch:
            if isinstance(item, SpeakerInput):
                segments = _segments_from_job(completed[str(item.source_id)])
                outputs.append(
                    SpeakerOutput(
                        source_id=item.source_id,
                        diarization=Diarization(
                            segments=[
                                DiarizationSegment(
                                    start_ms=start_ms,
                                    end_ms=end_ms,
                                    speaker=speaker,
                                )
                                for start_ms, end_ms, speaker in segments
                            ]
                        ),
                    )
                )
        return outputs

    def _upload_audio(self, audio: Any) -> str:
        media_url = f"media://batchalign/{uuid.uuid4().hex}.mp3"
        response = self._request_json(
            "POST", "/v1/media/input", payload={"url": media_url}
        )
        upload_url = response.get("url")
        if not isinstance(upload_url, str) or not upload_url.startswith("https://"):
            raise RuntimeError("pyannoteAI media endpoint returned no upload URL")

        request = urllib.request.Request(
            upload_url,
            data=self._media_renderer(audio),
            headers={"Content-Type": "application/octet-stream"},
            method="PUT",
        )
        self._open(request, operation="upload media")
        return media_url

    def _submit_job(self, media_url: str, num_speakers: int) -> str:
        payload: dict[str, Any] = {
            "url": media_url,
            "model": self._model,
        }
        if num_speakers > 0:
            payload["numSpeakers"] = num_speakers
        response = self._request_json("POST", "/v1/diarize", payload=payload)
        job_id = response.get("jobId")
        if not isinstance(job_id, str) or not job_id:
            raise RuntimeError("pyannoteAI diarize endpoint returned no jobId")
        return job_id

    def _poll_until_all_done(
        self, job_ids: dict[str, str]
    ) -> dict[str, dict[str, Any]]:
        pending = dict(job_ids)
        completed: dict[str, dict[str, Any]] = {}
        deadline = time.monotonic() + self._timeout

        while pending:
            for source_id, job_id in list(pending.items()):
                response = self._request_json("GET", f"/v1/jobs/{job_id}")
                status = str(response.get("status", "")).lower()
                if status not in _TERMINAL_STATUSES:
                    continue
                if status != "succeeded":
                    detail = response.get("message") or response.get("error") or status
                    raise RuntimeError(
                        f"pyannoteAI job {job_id} {status}: {detail}"
                    )
                completed[source_id] = response
                del pending[source_id]

            if not pending:
                break
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(
                    "pyannoteAI diarization timed out with "
                    f"{len(pending)} job(s) still running"
                )
            self._sleep(min(self._poll_interval, remaining))
        return completed

    def _request_json(
        self,
        method: str,
        path: str,
        *,
        payload: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        assert self._api_key is not None
        data = None if payload is None else json.dumps(payload).encode("utf-8")
        request = urllib.request.Request(
            f"{self._base_url}{path}",
            data=data,
            headers={
                "Authorization": f"Bearer {self._api_key}",
                "Content-Type": "application/json",
                "User-Agent": "batchalign/pyannote-ai",
            },
            method=method,
        )
        body = self._open(request, operation=f"{method} {path}")
        try:
            response = json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RuntimeError(
                f"pyannoteAI {method} {path} returned invalid JSON"
            ) from error
        if not isinstance(response, dict):
            raise RuntimeError(
                f"pyannoteAI {method} {path} returned an invalid response"
            )
        return response

    def _open(self, request: Any, *, operation: str) -> bytes:
        for attempt in range(4):
            try:
                with self._urlopen(request, timeout=self._http_timeout) as response:
                    return bytes(response.read())
            except urllib.error.HTTPError as error:
                if error.code == 429 and attempt < 3:
                    retry_after = (error.headers or {}).get("Retry-After", "1")
                    try:
                        delay = max(float(retry_after), 1.0)
                    except ValueError:
                        delay = 1.0
                    self._sleep(delay)
                    continue
                detail = _http_error_detail(error)
                raise RuntimeError(
                    f"pyannoteAI {operation} failed (HTTP {error.code}): {detail}"
                ) from error
            except (urllib.error.URLError, TimeoutError) as error:
                if request.get_method() in {"GET", "PUT"} and attempt < 3:
                    self._sleep(2**attempt)
                    continue
                raise RuntimeError(
                    f"pyannoteAI {operation} failed: {error}"
                ) from error
        raise RuntimeError(f"pyannoteAI {operation} failed after rate-limit retries")


def _render_mp3(audio: Any) -> bytes:
    """Render prepared speech PCM as a compact cloud-upload payload."""
    from batchalign._core.backends import ConvertBackend

    converter = ConvertBackend("mp3", mp3_bitrate_kbps=_CLOUD_MP3_BITRATE_KBPS)
    return bytes(
        converter.encode_prepared(
            bytes(audio.pcm_f32le),
            int(audio.sample_rate),
            int(audio.channels),
            int(audio.frame_count),
        )
    )


def _segments_from_job(job: dict[str, Any]) -> list[tuple[int, int, str]]:
    output = job.get("output")
    if not isinstance(output, dict):
        raise RuntimeError("pyannoteAI succeeded job has no output")
    warning = job.get("warning") or output.get("warning")
    if warning:
        _LOG.warning("pyannoteAI warning: %s", warning)
    raw_segments = output.get("diarization")
    if not isinstance(raw_segments, list):
        raise RuntimeError("pyannoteAI succeeded job has no diarization output")

    segments: list[tuple[int, int, str]] = []
    for raw_segment in raw_segments:
        if not isinstance(raw_segment, dict):
            continue
        try:
            start_ms = max(0, round(float(raw_segment["start"]) * 1000))
            end_ms = max(start_ms, round(float(raw_segment["end"]) * 1000))
            speaker = str(raw_segment["speaker"])
        except (KeyError, TypeError, ValueError):
            continue
        segments.append((start_ms, end_ms, speaker))
    segments.sort(key=lambda segment: (segment[0], segment[1], segment[2]))
    return segments


def _http_error_detail(error: urllib.error.HTTPError) -> str:
    try:
        body = error.read().decode("utf-8", errors="replace")
    except OSError:
        return str(error.reason)
    try:
        payload = json.loads(body)
    except json.JSONDecodeError:
        return body[:500] or str(error.reason)
    if isinstance(payload, dict):
        message = payload.get("message") or payload.get("error")
        if message:
            return str(message)
    return body[:500] or str(error.reason)


__all__ = ["PyannoteAIBackend"]
