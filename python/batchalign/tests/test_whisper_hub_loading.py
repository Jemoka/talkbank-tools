"""Tests for ``batchalign.inference.whisper_hub`` — HF fine-tune loader.

These tests cover the *dispatch* and *error* paths of the whisper_hub
loader without actually downloading a 1.5 GB model. The underlying HF
pipeline construction is exercised by the ML-golden suite separately.
"""

from __future__ import annotations

import pytest

from batchalign.device import DevicePolicy


class TestResolveWhisperHubModelId:
    """``resolve_whisper_hub_model_id`` picks model_id from lang or override."""

    def test_explicit_override_wins_over_language_default(self) -> None:
        from batchalign.inference.whisper_hub import resolve_whisper_hub_model_id

        resolved = resolve_whisper_hub_model_id(
            lang="mal",
            engine_overrides={"asr": "whisper_hub", "model_id": "other/mal-alt"},
        )
        assert resolved == "other/mal-alt"

    def test_language_default_is_used_when_no_override(self) -> None:
        from batchalign.inference.whisper_hub import resolve_whisper_hub_model_id

        resolved = resolve_whisper_hub_model_id(
            lang="mal",
            engine_overrides={"asr": "whisper_hub"},
        )
        assert resolved == "thennal/whisper-medium-ml"

    def test_no_default_and_no_override_raises_typed_error(self) -> None:
        from batchalign.inference.whisper_hub import (
            WhisperHubModelNotFoundError,
            resolve_whisper_hub_model_id,
        )

        # Language without a seeded default entry and no explicit model_id
        # must raise a typed error naming the language and pointing at the
        # escape hatch. No silent fallback to a random model.
        with pytest.raises(WhisperHubModelNotFoundError) as excinfo:
            resolve_whisper_hub_model_id(
                lang="xyz",
                engine_overrides={"asr": "whisper_hub"},
            )
        msg = str(excinfo.value)
        assert "xyz" in msg
        assert "model_id" in msg.lower()


class TestWhisperHubLoaderDispatch:
    """``load_whisper_hub_asr`` delegates through resolve_whisper_hub_model_id."""

    def test_loader_resolves_model_id_from_lang(self, monkeypatch) -> None:
        captured: dict[str, object] = {}

        class FakeHandle:
            """Minimal stand-in that accepts the ``skip_language_force``
            attribute write the loader performs. Checks that the loader
            actually flips it — not just trusting ``language="auto"``,
            because the V2 inference path ignores ``self.lang`` and
            passes the request language to ``gen_kwargs``.
            """

            def __init__(self) -> None:
                self.skip_language_force = False

        def fake_load_whisper_asr(*, model, base, language, device_policy):
            captured["model"] = model
            captured["base"] = base
            captured["language"] = language
            captured["device_policy"] = device_policy
            return FakeHandle()

        monkeypatch.setattr(
            "batchalign.inference.asr.load_whisper_asr",
            fake_load_whisper_asr,
        )

        from batchalign.inference.whisper_hub import load_whisper_hub_asr

        handle = load_whisper_hub_asr(
            lang="mal",
            engine_overrides={"asr": "whisper_hub"},
            device_policy=DevicePolicy(force_cpu=True),
        )

        assert isinstance(handle, FakeHandle)
        assert captured["model"] == "thennal/whisper-medium-ml"
        assert captured["base"] == "thennal/whisper-medium-ml"
        # Secondary safety belt — pinning ``self.lang == "auto"`` matters
        # for legacy code paths that read ``model.lang`` directly. The
        # V2 path doesn't consult ``self.lang``.
        assert captured["language"] == "auto"
        assert captured["device_policy"] == DevicePolicy(force_cpu=True)
        # Primary invariant — the V2 path calls ``gen_kwargs(request_lang)``
        # where request_lang is a concrete language like ``"malayalam"``.
        # Without this flag, ``gen_kwargs`` would pass task/language to
        # ``generate()`` and the fine-tune would emit gibberish.
        assert handle.skip_language_force is True
