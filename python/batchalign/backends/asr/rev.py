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

from batchalign.backends.base import ASR, UTR, Speaker, BatchPolicy
from batchalign import config
from batchalign.lang import LanguageCode


class RevAI(ASR, UTR, Speaker):
    """Rev.AI cloud ASR + diarization, atomic-call. Also serves `Task.Utr`."""

    def __init__(
        self,
        api_key: str | None = None,
        *,
        language: LanguageCode,
        num_speakers: int = 2,
        poll_interval_s: float = 5.0,
        timeout_s: float = 3600.0,
        batch_size: int = 8,
        batch_window_ms: int = 250,
    ) -> None:
        key = api_key if api_key is not None else config.get_api_key("revai", interactive=True)
        if not key:
            self._client = None
        else:
            from rev_ai import apiclient  # type: ignore[import-not-found]

            self._client = apiclient.RevAiAPIClient(key)
        self._poll = poll_interval_s
        self._timeout = timeout_s
        self._num_speakers = num_speakers
        # Rev.AI's `language=` field is ISO-639-1 alpha_2, with one
        # vendor quirk: Mandarin is `cmn`, not `zh`. The resolver
        # gave us alpha_2 (or alpha_3 if no alpha_2 exists, e.g.
        # `yue` for Cantonese — Rev accepts that as-is).
        self._language = _rev_code(language)
        # Submit-then-poll batching: stage all jobs first, then poll in
        # parallel so per-job latency is bounded by max(submission times)
        # + max(transcription times) instead of summed serially.
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # v4: punct-split + retrace for non-BERT languages (es, …). Bump when
        # submit/segmentation behaviour changes (cache key).
        return "revai:async-v4"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
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

        # Submit each unique source_id exactly once, then poll all in
        # parallel (Stanza-batching parity per the BA3 cutover plan).
        # Submission is cheap (one HTTP POST per file); the long wait is
        # Rev's server-side transcription. Polling sequentially after
        # batch-submission keeps each poll cycle covering every in-flight
        # job, so total wall time ≈ max(transcription_time) + small poll
        # overhead instead of sum(per-job transcription_time).
        unique_items: list[Any] = []
        seen: set[str] = set()
        for item in batch:
            if item.source_id in seen:
                continue
            seen.add(item.source_id)
            unique_items.append(item)

        job_ids = {
            it.source_id: self._submit(it.source_id, it.audio)
            for it in unique_items
        }
        responses: dict[str, dict[str, Any]] = self._poll_until_all_done(job_ids)

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

    def _submit(self, source_id: Any, audio: Any) -> str:
        """Upload the audio, return the Rev job ID (no polling).

        Splits the old _submit_and_wait so a whole batch can be uploaded
        before any polling starts (batch ASR per the BA3 cutover plan).
        Upload semantics unchanged from the original.
        """
        import os
        import tempfile
        from pathlib import Path

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

        submit_kwargs: dict[str, Any] = {"metadata": "batchalign"}
        if self._language:
            submit_kwargs["language"] = self._language
            if "en" in self._language or "es" in self._language:
                submit_kwargs["speakers_count"] = self._num_speakers
                submit_kwargs["skip_postprocessing"] = self._language in ("en", "fr")
        try:
            job = self._client.submit_job_local_file(upload_path, **submit_kwargs)
        finally:
            if tmp_path:
                try:
                    os.unlink(tmp_path)
                except OSError:
                    pass
        return str(job.id)

    def _poll_until_all_done(self, job_ids: dict[str, str]) -> dict[str, dict[str, Any]]:
        """Poll all submitted jobs in parallel until each is TRANSCRIBED.

        Maintains a `pending` dict; each poll cycle checks every job once
        and harvests any that completed. Total wall time ≈
        max(per-job transcription time) instead of the sum.
        """
        from rev_ai import JobStatus  # type: ignore[import-not-found]

        pending = dict(job_ids)  # source_id -> job_id
        results: dict[str, dict[str, Any]] = {}
        deadline = time.monotonic() + self._timeout
        while pending:
            for source_id, job_id in list(pending.items()):
                details = self._client.get_job_details(job_id)
                status = getattr(details, "status", None)
                if status == JobStatus.TRANSCRIBED:
                    results[source_id] = self._client.get_transcript_json(job_id)
                    del pending[source_id]
                elif status == JobStatus.FAILED:
                    raise RuntimeError(f"Rev.AI job {job_id} failed: {details!r}")
            if not pending:
                break
            if time.monotonic() > deadline:
                raise TimeoutError(
                    f"Rev.AI jobs did not finish in {self._timeout}s: "
                    f"still-pending={list(pending.values())}"
                )
            time.sleep(self._poll)
        return results

    def _submit_and_wait(self, source_id: Any, audio: Any) -> dict[str, Any]:
        """Single-job backwards-compat shim (calls _submit + _poll_until_all_done)."""
        job_id = self._submit(source_id, audio)
        return self._poll_until_all_done({str(source_id): job_id})[str(source_id)]


def _rev_code(lang: LanguageCode) -> str:
    """Resolved `LanguageCode` → Rev.AI's expected language code.

    Rev's API uses ISO-639-1 alpha_2; both `zho` (Chinese
    macrolanguage) and `cmn` (Mandarin) get sent as `cmn` because
    that's how Rev names Mandarin. Otherwise pass alpha_2 if known,
    falling back to alpha_3 (Cantonese / minority languages with no
    alpha_2 are sent as alpha_3, which Rev accepts for the ones it
    supports).
    """
    if lang.alpha_3 in ("zho", "cmn"):
        return "cmn"
    return lang.alpha_2_or_3


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
            # Rev tags `<silence>`, `<noise>`, etc. as type=text with the
            # angle-bracket marker as the value; those must be filtered or
            # the CHAT parser rejects them as unparseable main-tier
            # content (parity bug found 2026-05-31 vs BA2 utils.py:185).
            words = [
                AsrWord(
                    text=el.get("value", "").strip(),
                    start_ms=int((el.get("ts") or 0.0) * 1000),
                    end_ms=int((el.get("end_ts") or 0.0) * 1000),
                    confidence=el.get("confidence"),
                )
                for el in elements
                if el.get("type") == "text"
                and (val := el.get("value", "").strip())
                and not (val.startswith("<") and val.endswith(">"))
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
            # Reject Rev's `<silence>` / `<noise>` markers (BA2 utils.py:185
            # strips them with the same regex predicate).
            if (
                etype == "text"
                and value
                and not (value.startswith("<") and value.endswith(">"))
            ):
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
