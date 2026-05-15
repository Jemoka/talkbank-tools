"""Integration tests for HK engine providers.

These tests exercise the real load/infer function pairs against audio fixtures.
Each test auto-skips if its required dependencies or credentials are unavailable.

Fixtures (in tests/languages/cantonese/fixtures/):
  - 05b_clip.mp3  — 8-second Cantonese audio clip from 05b.cha (4.5s–12.5s)
  - 05b_clip.wav  — same clip, 16 kHz mono WAV (required by Aliyun)
  - 05b_clip.cha  — 4-utterance CHAT file matching the clip

Run:
  uv run pytest batchalign/tests/languages/cantonese/test_integration.py -v
  uv run pytest -m integration -v
  uv run pytest -m "not integration" -v
"""

from __future__ import annotations

import configparser
import pathlib
import re

import pytest

from batchalign.worker._types import BatchInferRequest, InferTask

from .conftest import CLIP_MP3, CLIP_WAV


# ---------------------------------------------------------------------------
# Dependency probes
# ---------------------------------------------------------------------------


def _has_funasr() -> bool:
    try:
        import funasr  # noqa: F401
        return True
    except ImportError:
        return False


def _has_pycantonese() -> bool:
    try:
        import pycantonese  # noqa: F401
        return True
    except ImportError:
        return False


def _has_opencc() -> bool:
    try:
        import opencc  # noqa: F401
        return True
    except ImportError:
        return False


def _has_tencent_sdk() -> bool:
    try:
        from tencentcloud.asr.v20190614.asr_client import AsrClient  # noqa: F401
        from qcloud_cos import CosS3Client  # noqa: F401
        return True
    except ImportError:
        return False


def _has_aliyun_sdk() -> bool:
    try:
        import nls  # noqa: F401
        from aliyunsdkcore.client import AcsClient  # noqa: F401
        return True
    except ImportError:
        return False


def _has_tencent_credentials() -> bool:
    cfg = configparser.ConfigParser()
    cfg.read(str(pathlib.Path.home() / ".batchalign.ini"))
    keys = (
        "engine.tencent.id",
        "engine.tencent.key",
        "engine.tencent.region",
        "engine.tencent.bucket",
    )
    return all(
        cfg.has_option("asr", k) and cfg.get("asr", k).strip() for k in keys
    )


def _has_aliyun_credentials() -> bool:
    cfg = configparser.ConfigParser()
    cfg.read(str(pathlib.Path.home() / ".batchalign.ini"))
    keys = (
        "engine.aliyun.ak_id",
        "engine.aliyun.ak_secret",
        "engine.aliyun.ak_appkey",
    )
    return all(
        cfg.has_option("asr", k) and cfg.get("asr", k).strip() for k in keys
    )


def _require_fixtures() -> bool:
    return CLIP_MP3.exists()


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_CJK_RE = re.compile(r"[\u4e00-\u9fff]")


def _make_asr_request(audio_path: str, lang: str = "yue") -> BatchInferRequest:
    """Build a BatchInferRequest with one AsrBatchItem."""
    from batchalign.inference.asr import AsrBatchItem

    item = AsrBatchItem(audio_path=audio_path, lang=lang, num_speakers=1)
    return BatchInferRequest(
        task=InferTask.ASR,
        lang=lang,
        items=[item.model_dump()],
    )


def _extract_tokens(response: object) -> list[dict[str, object]]:
    """Extract flat token dicts from the raw monologue worker payload.

    HK provider tests still assert text/timing/speaker behavior, but the
    production worker contract now returns tagged monologue payloads instead of
    a flattened shared token schema. This helper mirrors the Rust-side
    normalization only as far as these integration assertions need.
    """
    from batchalign.worker._types import BatchInferResponse
    from batchalign.inference.asr import MonologueAsrResponse

    assert isinstance(response, BatchInferResponse)
    assert len(response.results) == 1, f"Expected 1 result, got {len(response.results)}"
    result = response.results[0]
    assert result.error is None, f"Inference error: {result.error}"
    assert result.result is not None, "No result returned"

    asr = MonologueAsrResponse.model_validate(result.result)
    tokens: list[dict[str, object]] = []
    for monologue in asr.monologues:
        speaker = str(monologue.speaker)
        for element in monologue.elements:
            if element.type != "text":
                continue
            if not element.value.strip():
                continue
            tokens.append(
                {
                    "text": element.value,
                    "start_s": element.ts,
                    "end_s": element.end_ts,
                    "speaker": speaker,
                    "confidence": element.confidence,
                }
            )
    return tokens


# ---------------------------------------------------------------------------
# FunASR / SenseVoice
# ---------------------------------------------------------------------------


@pytest.mark.integration
@pytest.mark.skipif(not _has_funasr(), reason="funasr not installed")
@pytest.mark.skipif(not _has_opencc(), reason="opencc not installed")
@pytest.mark.skipif(not _require_fixtures(), reason="fixtures not found")
class TestFunAudioASR:
    @pytest.fixture(autouse=True)
    def _load_engine(self) -> None:
        from batchalign.inference.languages.cantonese._funaudio_asr import load_funaudio_asr, infer_funaudio_asr
        load_funaudio_asr("yue", None)
        self._infer = infer_funaudio_asr

    def test_produces_tokens(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        assert len(tokens) > 0

    def test_tokens_have_text(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        for t in tokens:
            assert str(t["text"]).strip(), f"Empty token text: {t}"

    def test_output_contains_cantonese(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        all_text = "".join(str(t["text"]) for t in tokens)
        assert _CJK_RE.search(all_text), f"No CJK characters in output: {all_text}"

    def test_tokens_have_timestamps(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        timed = [t for t in tokens if t.get("start_s") is not None]
        assert len(timed) > 0

    def test_normalization_applied(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        all_text = "".join(str(t["text"]) for t in tokens)
        for bad_char in ("系", "呀", "噶"):
            if bad_char in all_text:
                pytest.fail(
                    f"Unnormalized character '{bad_char}' found in output. "
                    f"Full text: {all_text}"
                )

    def test_elapsed_time_recorded(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        assert resp.results[0].elapsed_s > 0


# ---------------------------------------------------------------------------
# Tencent Cloud ASR
# ---------------------------------------------------------------------------


@pytest.mark.integration
@pytest.mark.skipif(not _has_tencent_sdk(), reason="Tencent SDK not installed")
@pytest.mark.skipif(not _has_tencent_credentials(), reason="Tencent credentials not found")
@pytest.mark.skipif(not _has_opencc(), reason="opencc not installed")
@pytest.mark.skipif(not _require_fixtures(), reason="fixtures not found")
class TestTencentASR:
    @pytest.fixture(autouse=True)
    def _load_engine(self) -> None:
        from batchalign.inference.languages.cantonese._tencent_asr import load_tencent_asr, infer_tencent_asr
        load_tencent_asr("yue", None)
        self._infer = infer_tencent_asr

    def test_produces_tokens(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        assert len(tokens) > 0

    def test_output_contains_cantonese(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        all_text = "".join(str(t["text"]) for t in tokens)
        assert _CJK_RE.search(all_text), f"No CJK in output: {all_text}"

    def test_tokens_have_timestamps(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        timed = [t for t in tokens if t.get("start_s") is not None]
        assert len(timed) > 0

    def test_normalization_applied(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        all_text = "".join(str(t["text"]) for t in tokens)
        assert "系" not in all_text or "係" in all_text, (
            f"Unnormalized '系' in output: {all_text}"
        )

    def test_has_speaker_info(self) -> None:
        req = _make_asr_request(str(CLIP_MP3))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        with_speaker = [t for t in tokens if t.get("speaker") is not None]
        assert len(with_speaker) > 0


# ---------------------------------------------------------------------------
# Aliyun NLS ASR
# ---------------------------------------------------------------------------


@pytest.mark.integration
@pytest.mark.skipif(not _has_aliyun_sdk(), reason="Aliyun SDK not installed")
@pytest.mark.skipif(not _has_aliyun_credentials(), reason="Aliyun credentials not found")
@pytest.mark.skipif(not _has_opencc(), reason="opencc not installed")
@pytest.mark.skipif(not _require_fixtures(), reason="fixtures not found")
class TestAliyunASR:
    @pytest.fixture(autouse=True)
    def _load_engine(self) -> None:
        from batchalign.inference.languages.cantonese._aliyun_asr import load_aliyun_asr, infer_aliyun_asr
        load_aliyun_asr("yue", None)
        self._infer = infer_aliyun_asr

    def test_produces_tokens(self) -> None:
        req = _make_asr_request(str(CLIP_WAV))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        assert len(tokens) > 0

    def test_output_contains_cantonese(self) -> None:
        req = _make_asr_request(str(CLIP_WAV))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        all_text = "".join(str(t["text"]) for t in tokens)
        assert _CJK_RE.search(all_text), f"No CJK in output: {all_text}"

    def test_tokens_have_timestamps_or_fallback(self) -> None:
        req = _make_asr_request(str(CLIP_WAV))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        assert len(tokens) > 0

    def test_normalization_applied(self) -> None:
        req = _make_asr_request(str(CLIP_WAV))
        resp = self._infer(req)
        tokens = _extract_tokens(resp)
        all_text = "".join(str(t["text"]) for t in tokens)
        assert "系" not in all_text or "係" in all_text

    def test_rejects_non_cantonese(self) -> None:
        from batchalign.inference.languages.cantonese._aliyun_asr import load_aliyun_asr
        with pytest.raises(ValueError, match="yue"):
            load_aliyun_asr("eng", None)


# ---------------------------------------------------------------------------
# Cantonese FA (jyutping + Wave2Vec)
# ---------------------------------------------------------------------------


@pytest.mark.integration
@pytest.mark.skipif(not _has_pycantonese(), reason="pycantonese not installed")
@pytest.mark.skipif(not _require_fixtures(), reason="fixtures not found")
class TestCantoneseFAProvider:
    @pytest.fixture(autouse=True)
    def _load_engine(self) -> None:
        from batchalign.inference.languages.cantonese._cantonese_fa import load_cantonese_fa, infer_cantonese_fa
        load_cantonese_fa("yue", None)
        self._infer = infer_cantonese_fa

    def test_empty_words_returns_empty_timings(self) -> None:
        from batchalign.inference.fa import FaInferItem

        item = FaInferItem(
            words=[],
            word_ids=[],
            word_utterance_indices=[],
            word_utterance_word_indices=[],
            audio_path=str(CLIP_MP3),
            audio_start_ms=0,
            audio_end_ms=1000,
        )
        req = BatchInferRequest(
            task=InferTask.FA,
            lang="yue",
            items=[item.model_dump()],
        )
        resp = self._infer(req)
        assert len(resp.results) == 1
        assert resp.results[0].error is None
        assert resp.results[0].result["indexed_timings"] == []

    def test_cantonese_words_produce_timings(self) -> None:
        from batchalign.inference.fa import FaInferItem, Wave2VecIndexedResponse

        item = FaInferItem(
            words=["咁", "搞", "笑", "嘅"],
            word_ids=["w0", "w1", "w2", "w3"],
            word_utterance_indices=[0, 0, 0, 0],
            word_utterance_word_indices=[0, 1, 2, 3],
            audio_path=str(CLIP_MP3),
            audio_start_ms=350,
            audio_end_ms=1375,
        )
        req = BatchInferRequest(
            task=InferTask.FA,
            lang="yue",
            items=[item.model_dump()],
        )
        resp = self._infer(req)
        assert len(resp.results) == 1
        result = resp.results[0]
        assert result.error is None, f"FA error: {result.error}"

        fa_resp = Wave2VecIndexedResponse.model_validate(result.result)
        assert len(fa_resp.indexed_timings) == 4
        non_none = [t for t in fa_resp.indexed_timings if t is not None]
        assert len(non_none) > 0


# ---------------------------------------------------------------------------
# Cantonese normalization (Rust-backed via batchalign_core)
# ---------------------------------------------------------------------------


class TestCantoneseNormalization:
    """Normalization is now pure Rust (embedded OpenCC + Aho-Corasick)."""

    def test_simplified_to_hk_traditional(self) -> None:
        from batchalign.inference.languages.cantonese._common import normalize_cantonese_text
        assert normalize_cantonese_text("联系") == "聯繫"

    def test_replacement_table(self) -> None:
        from batchalign.inference.languages.cantonese._common import normalize_cantonese_text
        assert normalize_cantonese_text("松") == "鬆"

    def test_idempotent_on_hk_text(self) -> None:
        from batchalign.inference.languages.cantonese._common import normalize_cantonese_text
        assert normalize_cantonese_text("你好") == "你好"

    def test_full_sentence_normalization(self) -> None:
        from batchalign.inference.languages.cantonese._common import normalize_cantonese_text
        assert normalize_cantonese_text("你真系好吵呀") == "你真係好嘈啊"
