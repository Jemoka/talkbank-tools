"""Aliyun NLS ASR provider for built-in HK/Cantonese engines.

New-style provider: a ``load`` function (validates credentials, stores
module-level state) and an ``infer`` function (processes a batch of ASR
items).  No engine class.

Aliyun NLS is **Cantonese-only** (``lang="yue"``).
"""

from __future__ import annotations

import configparser
import json
import logging
import pathlib
import tempfile
import time
import wave
from dataclasses import dataclass
from typing import Any

from pydantic import BaseModel, ValidationError

from batchalign.worker._types import (
    BatchInferRequest,
    BatchInferResponse,
    InferResponse,
)
from batchalign.inference.asr import (
    AsrBatchItem,
    MonologueAsrResponse,
)

from batchalign.inference._domain_types import AudioPath, LanguageCode

from ._common import (
    EngineOverrides,
    read_asr_config,
)

L = logging.getLogger("batchalign.hk.aliyun")


# ---------------------------------------------------------------------------
# Aliyun NLS response models (parse at the websocket boundary)
# ---------------------------------------------------------------------------


class AliyunWord(BaseModel):
    """One word from an Aliyun NLS SentenceEnd payload."""

    text: str
    startTime: int = 0
    endTime: int = 0


class _AliyunSentencePayload(BaseModel):
    """The ``payload`` object inside a SentenceEnd websocket message."""

    result: str = ""
    words: list[AliyunWord] = []


class _AliyunSentenceMessage(BaseModel):
    """Top-level Aliyun NLS SentenceEnd websocket message."""

    payload: _AliyunSentencePayload = _AliyunSentencePayload()


@dataclass
class AliyunSentenceResult:
    """Parsed result from one Aliyun SentenceEnd callback."""

    words: list[AliyunWord]
    sentence_text: str


# ---------------------------------------------------------------------------
# Module-level state (populated by load_aliyun_asr)
# ---------------------------------------------------------------------------

_ak_id: str = ""
_ak_secret: str = ""
_appkey: str = ""

# Cached Aliyun NLS token (valid for 24 hours)
_cached_token: str = ""
_cached_token_time: float = 0.0
_TOKEN_TTL_S: float = 23 * 3600  # refresh after 23 hours (1 hour safety margin)


# ---------------------------------------------------------------------------
# _AliyunRunner — websocket streaming transcription
# ---------------------------------------------------------------------------


class _AliyunRunner:
    """Streaming runner for Aliyun NLS websocket transcription."""

    def __init__(self, token: str, appkey: str, wav_path: str) -> None:
        self._token = token
        self._appkey = appkey
        self._wav_path = wav_path
        self._sample_rate = 16000
        self._audio_data = b""
        self._results: list[AliyunSentenceResult] = []

    def _load_file(self) -> None:
        with wave.open(self._wav_path, "rb") as wav_file:
            self._audio_data = wav_file.readframes(wav_file.getnframes())
            self._sample_rate = wav_file.getframerate()

    def _on_sentence_begin(self, _message: str, *_args: Any) -> None:
        return

    def _on_sentence_end(self, message: str, *_args: Any) -> None:
        parsed = _AliyunSentenceMessage.model_validate_json(message)
        self._results.append(
            AliyunSentenceResult(
                words=parsed.payload.words,
                sentence_text=parsed.payload.result.strip(),
            )
        )

    def _on_start(self, _message: str, *_args: Any) -> None:
        return

    def _on_error(self, message: str, *_args: Any) -> None:
        raise RuntimeError(f"Aliyun ASR error: {message}")

    def _on_close(self, *_args: Any) -> None:
        return

    def _on_result_changed(self, _message: str, *_args: Any) -> None:
        return

    def _on_completed(self, _message: str, *_args: Any) -> None:
        return

    def start(self) -> list[AliyunSentenceResult]:
        """Run streaming transcription, return parsed sentence results."""
        try:
            import nls
        except Exception as exc:
            raise ImportError(
                "Aliyun engine dependencies are missing from this "
                "environment. Reinstall batchalign3 or install the Aliyun "
                "SDK packages."
            ) from exc

        self._load_file()

        transcriber = nls.NlsSpeechTranscriber(
            url="wss://nls-gateway-ap-southeast-1.aliyuncs.com/ws/v1",
            token=self._token,
            appkey=self._appkey,
            on_sentence_begin=self._on_sentence_begin,
            on_sentence_end=self._on_sentence_end,
            on_start=self._on_start,
            on_result_changed=self._on_result_changed,
            on_completed=self._on_completed,
            on_error=self._on_error,
            on_close=self._on_close,
        )

        transcriber.start(
            aformat="pcm",
            enable_intermediate_result=True,
            enable_punctuation_prediction=False,
            enable_inverse_text_normalization=False,
            sample_rate=self._sample_rate,
            ex={"enable_words": True},
        )

        chunk_size = 640
        for offset in range(0, len(self._audio_data), chunk_size):
            transcriber.send_audio(self._audio_data[offset : offset + chunk_size])
            time.sleep(0.01)

        transcriber.ctrl(ex={"source": "batchalign3"})
        transcriber.stop()
        return self._results


# ---------------------------------------------------------------------------
# Credential helpers
# ---------------------------------------------------------------------------


def _get_token(ak_id: str, ak_secret: str) -> str:
    """Return a cached Aliyun NLS token, refreshing if expired (24h validity)."""
    global _cached_token, _cached_token_time

    now = time.monotonic()
    if _cached_token and (now - _cached_token_time) < _TOKEN_TTL_S:
        return _cached_token

    try:
        from aliyunsdkcore.client import AcsClient
        from aliyunsdkcore.request import CommonRequest
    except Exception as exc:
        raise ImportError(
            "Aliyun engine dependencies are missing from this "
            "environment. Reinstall batchalign3 or install the Aliyun SDK "
            "packages."
        ) from exc

    client = AcsClient(ak_id, ak_secret, "ap-southeast-1")
    request = CommonRequest()
    request.set_method("POST")
    request.set_domain("nlsmeta.ap-southeast-1.aliyuncs.com")
    request.set_version("2019-07-17")
    request.set_action_name("CreateToken")

    response = client.do_action_with_exception(request)
    payload = json.loads(response)

    token = payload.get("Token", {}).get("Id")
    if not isinstance(token, str) or not token:
        raise RuntimeError("Aliyun token request did not return Token.Id")

    _cached_token = token
    _cached_token_time = now
    L.info("Aliyun NLS token refreshed")
    return token


def _ensure_wav(source_path: str) -> tuple[str, tempfile.TemporaryDirectory[str] | None]:
    """Return a WAV path, converting if needed.  Caller must clean up temp_dir."""
    path = pathlib.Path(source_path)
    if path.suffix.lower() == ".wav":
        return source_path, None

    from batchalign.inference.audio import load_audio, save_audio

    audio, sample_rate = load_audio(source_path)
    if audio.dim() == 2 and audio.size(0) > 1:
        audio = audio.mean(dim=0, keepdim=True)

    temp_dir = tempfile.TemporaryDirectory(prefix="batchalign-hk-aliyun-")
    wav_path = pathlib.Path(temp_dir.name) / f"{path.stem}.wav"
    save_audio(wav_path, audio, sample_rate, bits_per_sample=16)
    return str(wav_path), temp_dir


# ---------------------------------------------------------------------------
# Provider load function
# ---------------------------------------------------------------------------


def load_aliyun_asr(
    lang: LanguageCode,
    engine_overrides: EngineOverrides | None,
    *,
    config: configparser.ConfigParser | None = None,
) -> None:
    """Validate Aliyun credentials and store them in module-level state.

    Aliyun NLS is Cantonese-only.  Raises ``ValueError`` if ``lang != "yue"``.
    Raises ``ConfigError`` if credentials are missing from ``~/.batchalign.ini``.
    """
    global _ak_id, _ak_secret, _appkey

    if lang != "yue":
        raise ValueError(
            "Aliyun ASR currently supports lang='yue' (Cantonese) only, "
            f"got lang={lang!r}."
        )

    creds = read_asr_config(
        (
            "engine.aliyun.ak_id",
            "engine.aliyun.ak_secret",
            "engine.aliyun.ak_appkey",
        ),
        engine="Aliyun",
        config=config,
    )

    _ak_id = creds["engine.aliyun.ak_id"]
    _ak_secret = creds["engine.aliyun.ak_secret"]
    _appkey = creds["engine.aliyun.ak_appkey"]
    L.info("Aliyun ASR credentials loaded (appkey=%s...)", _appkey[:4])


# ---------------------------------------------------------------------------
# Provider infer function
# ---------------------------------------------------------------------------


def _transcribe_to_monologues(audio_path: AudioPath) -> MonologueAsrResponse:
    """Run Aliyun websocket ASR on a single audio file and return monologues."""
    token = _get_token(_ak_id, _ak_secret)
    wav_path, temp_dir = _ensure_wav(audio_path)

    try:
        runner = _AliyunRunner(token=token, appkey=_appkey, wav_path=wav_path)
        results = runner.start()
    finally:
        if temp_dir is not None:
            temp_dir.cleanup()

    return _project_results(results)


def _project_results(results: list[AliyunSentenceResult]) -> MonologueAsrResponse:
    """Delegate Aliyun sentence projection to the shared Rust helper.

    The Python adapter keeps only websocket transport and shallow payload
    parsing. Rust owns the sentence-only fallback tokenization plus the shared
    monologue/timed-word projection rule used across HK ASR providers.
    """
    import batchalign_core

    projection = json.loads(
        batchalign_core.aliyun_sentences_to_asr(
            [
                {
                    "words": [word.model_dump() for word in result.words],
                    "sentence_text": result.sentence_text,
                }
                for result in results
            ],
            "yue",
        )
    )
    return MonologueAsrResponse.model_validate(
        {
            "kind": "monologues",
            "lang": "yue",
            "monologues": projection["monologues"],
        }
    )


def infer_aliyun_asr_v2(item: AsrBatchItem) -> MonologueAsrResponse:
    """Run one typed Aliyun ASR request for worker protocol V2.

    Aliyun still requires the websocket transport implemented in Python, but
    the live V2 worker path should call that transport directly instead of
    wrapping a single item in the batch-infer request shape.
    """

    return _transcribe_to_monologues(item.audio_path)


def infer_aliyun_asr(req: BatchInferRequest) -> BatchInferResponse:
    """Process a batch of ASR items via Aliyun NLS.

    Each item is an :class:`AsrBatchItem`. Returns a tagged monologue
    payload per item so Rust can own normalization and postprocessing.
    """
    if not _ak_id:
        raise RuntimeError("load_aliyun_asr() must be called before infer_aliyun_asr()")

    t0 = time.monotonic()
    n = len(req.items)
    results: list[InferResponse] = []

    for item_idx, raw_item in enumerate(req.items):
        try:
            item = AsrBatchItem.model_validate(raw_item)
        except ValidationError:
            results.append(InferResponse(error="Invalid AsrBatchItem", elapsed_s=0.0))
            continue

        try:
            response = _transcribe_to_monologues(item.audio_path)
            results.append(
                InferResponse(result=response.model_dump(), elapsed_s=0.0)
            )
        except Exception as e:
            L.warning(
                "Aliyun ASR failed for item %d (%s): %s",
                item_idx,
                item.audio_path,
                e,
                exc_info=True,
            )
            results.append(InferResponse(error=str(e), elapsed_s=0.0))

    elapsed = time.monotonic() - t0
    if results:
        first = results[0]
        results[0] = InferResponse(
            result=first.result, error=first.error, elapsed_s=elapsed,
        )

    L.info("batch_infer aliyun_asr: %d items, %.3fs", n, elapsed)
    return BatchInferResponse(results=results)
