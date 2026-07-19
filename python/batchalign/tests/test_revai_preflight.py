"""Hermetic regressions for Python-owned Rev.AI batch preflight."""

from __future__ import annotations

import base64
from types import MethodType


def _audio():
    from batchalign._core.proto import PreparedAudio

    return PreparedAudio(
        pcm_f32le=base64.b64encode(b"\x00" * 64).decode(),
        sample_rate=16_000,
        channels=1,
        frame_count=16,
    )


def test_revai_large_batch_submits_every_unique_job_before_polling():
    from batchalign._core.proto import AsrInput, AsrOptions, SpeakerInput
    from batchalign.backends.asr.rev import RevAI

    backend = RevAI.__new__(RevAI)
    backend._client = object()
    backend._language = "en"
    backend._num_speakers = 2

    events: list[tuple[str, object]] = []

    def submit(_self, source_id, _prepared_audio):
        events.append(("submit", source_id))
        return f"job-{source_id}"

    def poll(_self, job_ids):
        events.append(("poll", dict(job_ids)))
        return {source_id: {"monologues": []} for source_id in job_ids}

    backend._submit = MethodType(submit, backend)
    backend._poll_until_all_done = MethodType(poll, backend)

    audio = _audio()
    inputs = [
        AsrInput(
            source_id=f"audio-{index:02}",
            audio=audio,
            language={"kind": "code", "value": "eng"},
            options=AsrOptions(),
        )
        for index in range(32)
    ]
    # Atomic ASR + diarization projections for one source must share its
    # already-submitted provider job.
    inputs.append(SpeakerInput(source_id="audio-00", audio=audio, num_speakers=2))

    outputs = backend.call(inputs)

    assert len(outputs) == len(inputs)
    assert [kind for kind, _ in events[:-1]] == ["submit"] * 32
    assert events[-1][0] == "poll"
    poll_jobs = events[-1][1]
    assert isinstance(poll_jobs, dict)
    assert set(poll_jobs) == {f"audio-{index:02}" for index in range(32)}
    assert (
        sum(payload == "audio-00" for kind, payload in events if kind == "submit") == 1
    )
