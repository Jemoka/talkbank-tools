"""Gemini audio ASR with speaker diarization.

Gemini's Interactions API accepts uploaded audio and can return a structured
transcript containing timestamps and speaker labels.  Like :mod:`.rev`, this
is an atomic-call backend: one model interaction is shared by the ASR and
Speaker projections for a source.

API key resolution:

1. An explicit ``api_key=`` constructor argument
2. ``BATCHALIGN_GOOGLE_ASR_KEY``
3. ``~/.batchalign.ini`` ``[asr] engine.google.key``
"""

from __future__ import annotations

import json
import os
import tempfile
import time
import unicodedata
from typing import Any

from batchalign import config
from batchalign.backends.asr.rev import _pcm_to_wav_bytes
from batchalign.backends.base import ASR, UTR, BatchPolicy, Speaker
from batchalign.lang import LanguageCode


class GoogleGenAIBackend(ASR, UTR, Speaker):
    """Google Gemini cloud ASR + diarization via the Interactions API."""

    def __init__(
        self,
        api_key: str | None = None,
        *,
        language: LanguageCode,
        model: str = "gemini-3.5-flash",
        num_speakers: int = 2,
        batch_size: int = 8,
        batch_window_ms: int = 250,
        file_timeout_s: float = 300.0,
        client: Any = None,
    ) -> None:
        key = (
            api_key
            if api_key is not None
            else config.get_api_key("google_asr", interactive=True)
        )
        if client is not None:
            self._client = client
        elif key:
            from google import genai

            self._client = genai.Client(api_key=key)
        else:
            self._client = None
        self._language = language
        self._model = model
        self._num_speakers = num_speakers
        self._file_timeout = file_timeout_s
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"google-genai:{self._model}:json-v2"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(
        self, batch: list[Any], *, progress: Any = None, **_kwargs: Any
    ) -> list[Any]:
        from batchalign._core.proto import (
            AsrInput,
            AsrOutput,
            AsrSegment,
            AsrWord,
            Diarization,
            DiarizationSegment,
            SpeakerInput,
            SpeakerOutput,
        )

        if self._client is None:
            raise RuntimeError(
                "Google GenAI backend has no API key configured. Set "
                "BATCHALIGN_GOOGLE_ASR_KEY, or add "
                "`[asr] engine.google.key = ...` to ~/.batchalign.ini."
            )

        responses: dict[str, dict[str, Any]] = {}
        for item in batch:
            if item.source_id not in responses:
                responses[item.source_id] = self._transcribe(item.audio)

        outputs: list[Any] = []
        for item in batch:
            response = responses[item.source_id]
            if isinstance(item, AsrInput):
                outputs.append(
                    AsrOutput(
                        source_id=item.source_id,
                        segments=_asr_segments(
                            response,
                            AsrSegment,
                            AsrWord,
                            self._language.alpha_2_or_3,
                        ),
                    )
                )
            elif isinstance(item, SpeakerInput):
                outputs.append(
                    SpeakerOutput(
                        source_id=item.source_id,
                        diarization=Diarization(
                            segments=_diarization_segments(response, DiarizationSegment)
                        ),
                    )
                )
            else:
                raise TypeError(
                    f"Google GenAI does not handle input type: {type(item).__name__}"
                )
        return outputs

    def _transcribe(self, audio: Any) -> dict[str, Any]:
        prompt = (
            "Transcribe all speech in this audio verbatim in "
            f"{self._language.name} ({self._language.alpha_3}). "
            "Separate the transcript into natural utterance-sized segments. "
            "Identify each distinct speaker consistently using short labels "
            "such as 0, 1, and 2. "
            f"There are approximately {self._num_speakers} speakers; treat "
            "that as a hint, not a requirement. Give each segment's start and "
            "end timestamp from the beginning of the audio using MM:SS.mmm. "
            "Do not translate, summarize, or include non-speech event "
            "descriptions. Return only a valid JSON object with a segments "
            "array. Every segment must contain the string fields "
            "start_timestamp, end_timestamp, speaker, and content."
        )

        tmp_path = ""
        uploaded_file: Any = None
        try:
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                tmp.write(_pcm_to_wav_bytes(audio))
                tmp_path = tmp.name
            uploaded_file = self._client.files.upload(
                file=tmp_path, config={"mime_type": "audio/wav"}
            )
            uploaded_file = self._wait_until_active(uploaded_file)
            interaction = self._client.interactions.create(
                model=self._model,
                input=[
                    {"type": "text", "text": prompt},
                    {
                        "type": "audio",
                        "uri": uploaded_file.uri,
                        # Files currently reports WAV uploads as audio/x-wav,
                        # but Interactions accepts only the canonical spelling.
                        "mime_type": "audio/wav",
                    },
                ],
            )
            return _parse_response(interaction.output_text)
        finally:
            if uploaded_file is not None:
                name = getattr(uploaded_file, "name", None)
                if name:
                    try:
                        self._client.files.delete(name=name)
                    except Exception:
                        # Remote files expire automatically; cleanup must not
                        # hide a successful transcript or the original error.
                        pass
            if tmp_path:
                try:
                    os.unlink(tmp_path)
                except OSError:
                    pass

    def _wait_until_active(self, uploaded_file: Any) -> Any:
        """Wait until the Files API has made an upload usable by Interactions."""
        deadline = time.monotonic() + self._file_timeout
        while _state_name(getattr(uploaded_file, "state", None)) == "PROCESSING":
            if time.monotonic() > deadline:
                raise TimeoutError(
                    f"Google audio upload did not become active in {self._file_timeout}s"
                )
            time.sleep(1.0)
            uploaded_file = self._client.files.get(name=uploaded_file.name)
        state = _state_name(getattr(uploaded_file, "state", None))
        if state == "FAILED":
            raise RuntimeError(
                f"Google audio upload failed: {getattr(uploaded_file, 'error', None)}"
            )
        return uploaded_file


def _state_name(state: Any) -> str:
    return str(getattr(state, "name", state) or "").upper()


def _parse_response(output_text: str) -> dict[str, Any]:
    """Parse and minimally validate Gemini's structured JSON response."""
    text = output_text.strip()
    if text.startswith("```"):
        first_newline = text.find("\n")
        if first_newline >= 0:
            text = text[first_newline + 1 :]
        if text.endswith("```"):
            text = text[:-3].rstrip()
    try:
        response = json.loads(text)
    except (TypeError, json.JSONDecodeError) as exc:
        raise RuntimeError("Google GenAI returned invalid transcript JSON") from exc
    if not isinstance(response, dict) or not isinstance(response.get("segments"), list):
        raise RuntimeError("Google GenAI transcript JSON has no segments array")
    normalized: list[dict[str, Any]] = []
    for raw in response["segments"]:
        if not isinstance(raw, dict):
            continue
        normalized.append(
            {
                "start_ms": _timestamp_to_ms(str(raw.get("start_timestamp", "0"))),
                "end_ms": _timestamp_to_ms(str(raw.get("end_timestamp", "0"))),
                "speaker": str(raw.get("speaker", "0")),
                "text": str(raw.get("content", "")),
                "words": [],
            }
        )
    return {"segments": normalized}


def _timestamp_to_ms(timestamp: str) -> int:
    """Convert Gemini's ``MM:SS.mmm`` (or ``HH:MM:SS.mmm``) to milliseconds."""
    try:
        parts = [float(part.strip()) for part in timestamp.strip().split(":")]
    except ValueError as exc:
        raise RuntimeError(f"Google GenAI returned invalid timestamp {timestamp!r}") from exc
    if not 1 <= len(parts) <= 3:
        raise RuntimeError(f"Google GenAI returned invalid timestamp {timestamp!r}")
    seconds = 0.0
    for part in parts:
        seconds = seconds * 60 + part
    return max(0, round(seconds * 1000))


def _asr_segments(
    response: dict[str, Any], AsrSegment: type, AsrWord: type, lang: str = ""
) -> list[Any]:
    from batchalign.backends.utseg.cleanup import clean_utterance, load_cleanup

    cleanup_table = load_cleanup(lang)
    segments: list[Any] = []
    for raw in response.get("segments", []):
        text = _chat_safe_text(str(raw.get("text", "")))
        text = clean_utterance(text, cleanup_table, lang)
        words = []
        for word in raw.get("words", []):
            word_text = str(word.get("text", "")).strip()
            if not word_text:
                continue
            start_ms = max(0, int(word.get("start_ms", 0)))
            end_ms = max(start_ms, int(word.get("end_ms", start_ms)))
            words.append(
                AsrWord(
                    text=word_text,
                    start_ms=start_ms,
                    end_ms=end_ms,
                    confidence=word.get("confidence"),
                )
            )
        if not text and words:
            text = " ".join(word.text for word in words)
        if not text:
            continue
        start_ms = max(0, int(raw.get("start_ms", 0)))
        end_ms = max(start_ms, int(raw.get("end_ms", start_ms)))
        segments.append(
            AsrSegment(
                start_ms=start_ms,
                end_ms=end_ms,
                text=text,
                speaker=str(raw.get("speaker", "0")),
                words=words,
            )
        )
    return segments


def _chat_safe_text(text: str) -> str:
    """Drop model punctuation CHAT reserves for its own tier syntax."""
    chars: list[str] = []
    for index, char in enumerate(text):
        if char in {"'", "’", "-"}:
            before = index > 0 and text[index - 1].isalnum()
            after = index + 1 < len(text) and text[index + 1].isalnum()
            if before and after:
                chars.append("'" if char == "’" else char)
            else:
                chars.append(" ")
        elif unicodedata.category(char)[:1] in {"P", "S"}:
            chars.append(" ")
        else:
            chars.append(char)
    return " ".join("".join(chars).split())


def _diarization_segments(
    response: dict[str, Any], DiarizationSegment: type
) -> list[Any]:
    """Project transcript segments into merged, non-overlapping speaker spans."""
    merged: list[dict[str, Any]] = []
    for raw in response.get("segments", []):
        start_ms = max(0, int(raw.get("start_ms", 0)))
        end_ms = max(start_ms, int(raw.get("end_ms", start_ms)))
        if end_ms == start_ms:
            continue
        speaker = str(raw.get("speaker", "0"))
        if (
            merged
            and merged[-1]["speaker"] == speaker
            and start_ms <= merged[-1]["end_ms"] + 50
        ):
            merged[-1]["end_ms"] = max(merged[-1]["end_ms"], end_ms)
        else:
            merged.append({"start_ms": start_ms, "end_ms": end_ms, "speaker": speaker})
    return [DiarizationSegment(**segment) for segment in merged]


__all__ = ["GoogleGenAIBackend"]
