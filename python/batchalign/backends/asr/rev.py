"""RevAI: atomic-call ASR + Speaker diarization.

Rev.AI is the canonical "atomic-call" backend — one submission yields
both the ASR transcript with word timestamps and speaker diarization.
The engine batches it at :meth:`BatchPolicy.one` (one job per audio);
the backend dedupes by ``source_id`` and projects the single response
into either :class:`AsrOutput` or :class:`SpeakerOutput` depending on
which input variant arrived.

API key resolution (see :mod:`batchalign.config`):

1. ``BATCHALIGN_REVAI_KEY`` env var
2. ``~/.batchalign.ini`` ``[asr] engine.rev.key``
"""

from __future__ import annotations

import io
import time
import wave
from typing import Any

from batchalign.backends.base import ASR, Speaker, BatchPolicy
from batchalign import config


class RevAI(ASR, Speaker):
    """Rev.AI cloud ASR + diarization, atomic-call."""

    def __init__(
        self,
        api_key: str | None = None,
        *,
        language: str | None = None,
        num_speakers: int = 2,
        poll_interval_s: float = 5.0,
        timeout_s: float = 3600.0,
    ) -> None:
        key = api_key if api_key is not None else config.get_api_key("revai")
        if not key:
            self._client = None
        else:
            from rev_ai import apiclient  # type: ignore[import-not-found]

            self._client = apiclient.RevAiAPIClient(key)
        self._poll = poll_interval_s
        self._timeout = timeout_s
        self._num_speakers = num_speakers
        # Rev.AI language code (BA2 maps ISO-639-3 → -1, zho→cmn). `None`/"auto"
        # lets Rev pick its default (English).
        self._language = _rev_lang(language)
        self._policy = BatchPolicy.one()

    @property
    def name(self) -> str:
        # v4: punct-split + retrace for non-BERT languages (es, …). Bump when
        # submit/segmentation behaviour changes (cache key).
        return "revai:async-v4"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import (
            AsrInput,
            AsrOutput,
            AsrSegment,
            AsrWord,
            SpeakerInput,
            SpeakerOutput,
            Diarization,
            DiarizationSegment,
        )

        if self._client is None:
            raise RuntimeError(
                "RevAI backend has no API key configured. Set "
                "BATCHALIGN_REVAI_KEY or add `[asr] engine.rev.key = ...` "
                "to ~/.batchalign.ini."
            )

        # Submit each unique source_id exactly once; project results.
        responses: dict[str, dict[str, Any]] = {}
        for item in batch:
            if item.source_id in responses:
                continue
            responses[item.source_id] = self._submit_and_wait(item.source_id, item.audio)

        outputs: list[Any] = []
        for item in batch:
            resp = responses[item.source_id]
            if isinstance(item, AsrInput):
                outputs.append(
                    AsrOutput(
                        source_id=item.source_id,
                        segments=_segments_from_rev(resp, AsrSegment, AsrWord, self._language),
                    )
                )
            elif isinstance(item, SpeakerInput):
                outputs.append(
                    SpeakerOutput(
                        source_id=item.source_id,
                        diarization=Diarization(
                            segments=_diar_from_rev(resp, DiarizationSegment),
                        ),
                    )
                )
            else:
                raise TypeError(
                    f"RevAI does not handle input type: {type(item).__name__}"
                )
        return outputs

    # ----- HTTP submission ----------------------------------------------

    def _submit_and_wait(self, source_id: Any, audio: Any) -> dict[str, Any]:
        """Upload the audio, poll until ``transcribed``, return parsed JSON.

        Uploads the ORIGINAL media file when `source_id` is a real path (Rev
        gets byte-identical audio to what BA2 sends, so the transcript is the
        same); otherwise stages a WAV from the decoded PCM. Re-encoding PCM
        can perturb Rev's output on tone-sensitive audio (e.g. Mandarin), so
        the original-file path is the parity path.

        Submit options mirror BA2's `RevEngine`: pass `language`, and (only for
        en/es, which Rev allows) `speakers_count` + `skip_postprocessing`
        (True for en/fr, where BA2 re-segments with CHATUtterance).
        """
        import os
        import tempfile
        from pathlib import Path
        from rev_ai import JobStatus  # type: ignore[import-not-found]

        # Prefer the original media file (byte-identical to BA2's upload).
        orig = str(source_id) if source_id is not None else ""
        tmp_path = ""
        if orig and Path(orig).is_file():
            upload_path = orig
        else:
            wav_bytes = _pcm_to_wav_bytes(audio)
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                tmp.write(wav_bytes)
                tmp_path = tmp.name
            upload_path = tmp_path

        # Mirror BA2's conditional submit: `speakers_count` (and
        # `skip_postprocessing`) are only accepted by Rev for en/es/fr/pt, and
        # BA2 only sets them when the language contains "en" or "es". For other
        # languages (cmn, yue, …) it sends just `language`.
        submit_kwargs: dict[str, Any] = {"metadata": "batchalign"}
        if self._language:
            submit_kwargs["language"] = self._language
            if "en" in self._language or "es" in self._language:
                submit_kwargs["speakers_count"] = self._num_speakers
                # Skip Rev's own postproc only where BA2 re-segments (en/fr).
                submit_kwargs["skip_postprocessing"] = self._language in ("en", "fr")
        try:
            job = self._client.submit_job_local_file(upload_path, **submit_kwargs)
        finally:
            if tmp_path:
                try:
                    os.unlink(tmp_path)
                except OSError:
                    pass

        deadline = time.monotonic() + self._timeout
        while True:
            details = self._client.get_job_details(job.id)
            status = getattr(details, "status", None)
            if status == JobStatus.TRANSCRIBED:
                break
            if status == JobStatus.FAILED:
                raise RuntimeError(f"Rev.AI job {job.id} failed: {details!r}")
            if time.monotonic() > deadline:
                raise TimeoutError(f"Rev.AI job {job.id} did not finish in {self._timeout}s")
            time.sleep(self._poll)
        return self._client.get_transcript_json(job.id)


def _rev_lang(code: str | None) -> str | None:
    """Map a CHAT/ISO language code to Rev.AI's code (BA2's mapping).

    `None`/"auto" → `None` (Rev defaults to English). ISO-639-3 is mapped to
    -1 via pycountry; Mandarin (`zho`/`zh`) becomes `cmn`, which is what Rev
    expects (BA2 `asr/rev.py`).
    """
    if not code or code == "auto":
        return None
    c = code.strip().lower()
    if c in ("zho", "zh", "cmn", "zh-hans", "zh-hant"):
        return "cmn"
    if len(c) <= 2:
        return c
    try:
        import pycountry  # type: ignore[import-not-found]

        lang = pycountry.languages.get(alpha_3=c)
        if lang is not None and getattr(lang, "alpha_2", None):
            return lang.alpha_2
    except Exception:
        pass
    return c


# ---------------------------------------------------------------------------
# Pure functions — easy to unit-test against a recorded Rev.AI JSON response.
# ---------------------------------------------------------------------------


_SENT_END = {".", "?", "!"}
# Rev language codes that have a downstream CHATUtterance BERT segmenter
# (en/zh/yue). For these, BA2 strips Rev's punctuation and segments with the
# BERT model; everything else uses Rev's punctuation to sentence-split.
_BERT_LANGS = {"en", "cmn", "yue"}


def _segments_from_rev(
    resp: dict[str, Any], AsrSegment: type, AsrWord: type, lang: str | None = None
) -> list[Any]:
    """Project a Rev.AI transcript JSON into a list of :class:`AsrSegment`.

    Rev's JSON is ``{"monologues": [{"speaker": int, "elements":
    [{"type": "text"|"punct", "value": "...", "ts": float, "end_ts": float},
    ...]}, ...]}``.

    Two modes, mirroring BA2's `process_generation`:

    * **Punctuated** (Rev's own postproc kept — non en/fr): Rev returns commas
      and sentence-final periods. We split each monologue into sentences at
      `. ? !` (keeping commas inline), apply BA2's disfluency + retrace cleanup
      per sentence, and emit ONE segment per sentence — BA2's `retokenize`.
    * **Raw** (en/fr, `skip_postprocessing=True`): no Rev punctuation; emit the
      whole monologue as one segment for the downstream CHATUtterance BERT
      segmenter to carve.
    """
    from batchalign.backends.utseg.cleanup import (
        SUPPORT_SUFFIX,
        clean_utterance,
        load_cleanup,
    )

    suffix = SUPPORT_SUFFIX.get((lang or "").lower(), (lang or "").lower())
    table = load_cleanup(suffix)

    use_bert = (lang or "").lower() in _BERT_LANGS

    segments: list[Any] = []
    for mono in resp.get("monologues", []):
        speaker = str(mono.get("speaker")) if mono.get("speaker") is not None else None
        elements = mono.get("elements", [])
        has_sentence_punct = any(
            el.get("type") == "punct" and (el.get("value") or "").strip() in _SENT_END
            for el in elements
        )

        # BERT-segmented languages (en/zh/yue) strip Rev punctuation and let the
        # downstream CHATUtterance segmenter split; only punct-split when there
        # is no BERT model AND Rev gave us sentence punctuation.
        if use_bert or not has_sentence_punct:
            # Raw monologue → one segment (BERT segmenter handles the rest).
            words = [
                AsrWord(
                    text=el.get("value", "").strip(),
                    start_ms=int((el.get("ts") or 0.0) * 1000),
                    end_ms=int((el.get("end_ts") or 0.0) * 1000),
                    confidence=el.get("confidence"),
                )
                for el in elements
                if el.get("type") == "text" and el.get("value", "").strip()
            ]
            if not words:
                continue
            segments.append(
                AsrSegment(
                    start_ms=words[0].start_ms,
                    end_ms=words[-1].end_ms,
                    text=" ".join(w.text for w in words),
                    speaker=speaker,
                    words=words,
                )
            )
            continue

        # Punctuated → split into sentences at . ? ! (commas stay inline).
        sentence: list[Any] = []  # AsrWord, includes comma tokens

        def _flush(sent: list[Any]) -> None:
            toks = [w for w in sent if w.text]
            if not toks:
                return
            raw = " ".join(w.text for w in toks)
            cleaned = clean_utterance(raw, table, lang or "")
            timed = [w for w in toks if w.text not in (",",)]
            start = timed[0].start_ms if timed else toks[0].start_ms
            end = timed[-1].end_ms if timed else toks[-1].end_ms
            segments.append(
                AsrSegment(
                    start_ms=start, end_ms=end, text=cleaned, speaker=speaker,
                    words=toks,
                )
            )

        for el in elements:
            value = (el.get("value") or "").strip()
            etype = el.get("type")
            if etype == "punct":
                if value in _SENT_END:
                    _flush(sentence)
                    sentence = []
                elif value:  # comma etc. — keep inline
                    sentence.append(AsrWord(text=value, start_ms=0, end_ms=0, confidence=None))
                continue
            if etype == "text" and value:
                sentence.append(
                    AsrWord(
                        text=value,
                        start_ms=int((el.get("ts") or 0.0) * 1000),
                        end_ms=int((el.get("end_ts") or 0.0) * 1000),
                        confidence=el.get("confidence"),
                    )
                )
        _flush(sentence)
    return segments


def _diar_from_rev(
    resp: dict[str, Any], DiarizationSegment: type
) -> list[Any]:
    """Project Rev.AI monologues into diarization spans."""
    segs: list[Any] = []
    for mono in resp.get("monologues", []):
        elements = [e for e in mono.get("elements", []) if e.get("type") == "text"]
        if not elements:
            continue
        start_ms = int((elements[0].get("ts") or 0.0) * 1000)
        end_ms = int((elements[-1].get("end_ts") or 0.0) * 1000)
        segs.append(
            DiarizationSegment(
                start_ms=start_ms,
                end_ms=end_ms,
                speaker=str(mono.get("speaker", "0")),
            )
        )
    return segs


def _pcm_to_wav_bytes(audio: Any) -> bytes:
    """Encode PCM-float32 ``audio`` as a 16-bit mono WAV byte string."""
    import numpy as np  # type: ignore[import-not-found]

    arr = np.frombuffer(audio.pcm_f32le, dtype=np.float32)
    pcm16 = (np.clip(arr, -1.0, 1.0) * 32767.0).astype(np.int16)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(int(audio.sample_rate))
        wf.writeframes(pcm16.tobytes())
    return buf.getvalue()


__all__ = ["RevAI"]
