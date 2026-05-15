"""Tests for the staged worker protocol V2 prepared-artifact readers."""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pytest

from batchalign.worker._artifact_inputs_v2 import (
    ArtifactInputErrorV2,
    find_attachment_by_id_v2,
    load_json_attachment_v2,
    load_prepared_audio_f32le_v2,
    load_prepared_text_json_v2,
    require_attachment_type_v2,
)
from batchalign.worker._types_v2 import (
    InlineJsonRefV2,
    PreparedAudioEncodingV2,
    PreparedAudioRefV2,
    PreparedTextEncodingV2,
    PreparedTextRefV2,
)


def _write_pcm_f32le(path: Path, samples: np.ndarray) -> None:
    """Write little-endian float32 PCM test data to disk."""

    path.write_bytes(samples.astype("<f4").tobytes())


def test_load_json_attachment_from_inline_payload() -> None:
    """Inline JSON attachments should deserialize without touching the filesystem."""

    attachment = InlineJsonRefV2(id="inline-ref-1", value={"words": ["hello", "world"]})
    assert load_json_attachment_v2([attachment], "inline-ref-1") == {
        "words": ["hello", "world"]
    }


def test_load_prepared_text_json_payload(tmp_path: Path) -> None:
    """Prepared text descriptors should read only the declared JSON slice."""

    payload_path = tmp_path / "payload.json"
    payload_path.write_text('ignore-me{"words":["alpha","beta"]}tail', encoding="utf-8")
    attachment = PreparedTextRefV2(
        id="payload-ref-1",
        path=str(payload_path),
        encoding=PreparedTextEncodingV2.UTF8_JSON,
        byte_offset=len("ignore-me"),
        byte_len=len('{"words":["alpha","beta"]}'),
    )

    assert load_prepared_text_json_v2(attachment) == {"words": ["alpha", "beta"]}
    assert load_json_attachment_v2([attachment], "payload-ref-1") == {
        "words": ["alpha", "beta"]
    }


def test_load_prepared_audio_f32le_payload(tmp_path: Path) -> None:
    """Prepared PCM audio should roundtrip into a detached numpy array."""

    audio_path = tmp_path / "audio.pcm"
    samples = np.asarray([0.25, -0.5, 1.0, 0.0], dtype=np.float32)
    _write_pcm_f32le(audio_path, samples)
    attachment = PreparedAudioRefV2(
        id="audio-ref-1",
        path=str(audio_path),
        encoding=PreparedAudioEncodingV2.PCM_F32LE,
        channels=1,
        sample_rate_hz=16000,
        frame_count=4,
        byte_offset=0,
        byte_len=16,
    )

    loaded = load_prepared_audio_f32le_v2(attachment)
    assert loaded.shape == (4,)
    assert np.allclose(loaded, samples)
    assert loaded.flags["OWNDATA"]


def test_find_attachment_by_id_requires_presence() -> None:
    """Attachment lookup should fail with a clear error when the id is absent."""

    with pytest.raises(ArtifactInputErrorV2, match="missing worker protocol V2 attachment"):
        find_attachment_by_id_v2([], "missing-ref")


def test_find_attachment_by_id_roundtrips_descriptor_shape() -> None:
    """Attachment lookup should preserve the typed descriptor shape."""

    attachment = InlineJsonRefV2(id="inline-ref-found", value={"ok": True})
    found = find_attachment_by_id_v2([attachment], "inline-ref-found")

    assert isinstance(found, InlineJsonRefV2)
    assert found.value == {"ok": True}


def test_require_attachment_type_rejects_wrong_shape() -> None:
    """Type-specific readers should reject attachments of the wrong descriptor class."""

    attachment = InlineJsonRefV2(id="inline-ref-2", value={"ok": True})
    with pytest.raises(ArtifactInputErrorV2, match="expected PreparedAudioRefV2"):
        require_attachment_type_v2([attachment], "inline-ref-2", PreparedAudioRefV2)


def test_prepared_audio_reader_rejects_inconsistent_length(tmp_path: Path) -> None:
    """Prepared audio descriptors should validate byte length against frame metadata."""

    audio_path = tmp_path / "bad-audio.pcm"
    _write_pcm_f32le(audio_path, np.asarray([0.1, 0.2], dtype=np.float32))
    attachment = PreparedAudioRefV2(
        id="audio-ref-2",
        path=str(audio_path),
        encoding=PreparedAudioEncodingV2.PCM_F32LE,
        channels=1,
        sample_rate_hz=16000,
        frame_count=4,
        byte_offset=0,
        byte_len=8,
    )

    with pytest.raises(ArtifactInputErrorV2, match="expected 16"):
        load_prepared_audio_f32le_v2(attachment)


def test_prepared_audio_reader_rejects_zero_sample_rate_when_validation_is_bypassed(
    tmp_path: Path,
) -> None:
    """Rust should reject numerically invalid prepared-audio descriptors too."""

    audio_path = tmp_path / "zero-rate-audio.pcm"
    _write_pcm_f32le(audio_path, np.asarray([0.1, 0.2], dtype=np.float32))
    attachment = PreparedAudioRefV2.model_construct(
        id="audio-ref-zero-rate",
        path=str(audio_path),
        encoding=PreparedAudioEncodingV2.PCM_F32LE,
        channels=1,
        sample_rate_hz=0,
        frame_count=2,
        byte_offset=0,
        byte_len=8,
    )

    with pytest.raises(ArtifactInputErrorV2, match="positive sample_rate_hz"):
        load_prepared_audio_f32le_v2(attachment)


def test_prepared_text_reader_rejects_out_of_bounds_slice(tmp_path: Path) -> None:
    """Prepared text descriptors should not silently read beyond their file bounds."""

    payload_path = tmp_path / "payload.json"
    payload_path.write_text(json.dumps({"value": 1}), encoding="utf-8")
    attachment = PreparedTextRefV2(
        id="payload-ref-2",
        path=str(payload_path),
        encoding=PreparedTextEncodingV2.UTF8_JSON,
        byte_offset=10,
        byte_len=50,
    )

    with pytest.raises(ArtifactInputErrorV2, match="outside"):
        load_prepared_text_json_v2(attachment)
