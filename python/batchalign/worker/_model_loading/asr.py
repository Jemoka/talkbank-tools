"""ASR-engine bootstrap helpers for worker startup."""

from __future__ import annotations

import logging
import os
from collections.abc import Mapping

from batchalign.inference._domain_types import RevAiApiKey
from batchalign.inference.asr import iso3_to_language_name
from batchalign.worker._types import AsrEngine, WorkerBootstrapRuntime, _state

L = logging.getLogger("batchalign.worker")


def load_asr_engine(bootstrap: WorkerBootstrapRuntime) -> None:
    """Load the ASR engine for this worker.

    The control plane may inject a resolved Rev.AI key directly into the worker
    bootstrap runtime. When it does, that injected value is authoritative and
    the worker does not rediscover credentials from ambient process state.
    """
    lang = bootstrap.lang
    engine_overrides = bootstrap.engine_overrides or None
    rev_api_key = bootstrap.revai_api_key
    _state.rev_api_key = None

    asr_engine = resolve_asr_engine(engine_overrides, rev_api_key)

    if asr_engine == "rev":
        _state.rev_api_key = rev_api_key
        if rev_api_key is None:
            L.error("Rev.AI key not configured")
        _state.asr_engine = AsrEngine.REV
    elif asr_engine == "tencent":
        from batchalign.inference.languages.cantonese._tencent_asr import load_tencent_asr

        load_tencent_asr(lang, engine_overrides)
        _state.asr_engine = AsrEngine.TENCENT
    elif asr_engine == "aliyun":
        from batchalign.inference.languages.cantonese._aliyun_asr import load_aliyun_asr

        load_aliyun_asr(lang, engine_overrides)
        _state.asr_engine = AsrEngine.ALIYUN
    elif asr_engine == "funaudio":
        from batchalign.inference.languages.cantonese._funaudio_asr import load_funaudio_asr

        load_funaudio_asr(lang, engine_overrides)
        _state.asr_engine = AsrEngine.FUNAUDIO
    elif asr_engine == "whisper_hub":
        # Community HF Whisper fine-tune loaded by model_id. Resolution
        # and the "unknown language" error path live in
        # ``batchalign.inference.whisper_hub``; the returned handle is
        # the same ``WhisperASRHandle`` stock Whisper uses, so downstream
        # V2 inference requires no branching on engine identity after
        # load time.
        from batchalign.inference.whisper_hub import load_whisper_hub_asr

        _state.whisper_asr_model = load_whisper_hub_asr(
            lang,
            engine_overrides,
            device_policy=bootstrap.device_policy,
        )
        _state.asr_engine = AsrEngine.WHISPER_HUB
    else:
        from batchalign.inference.asr import load_whisper_asr

        language = iso3_to_language_name(lang)
        # When auto-detecting, always use the multilingual model with no
        # language-specific overrides so Whisper detects per-segment.
        if language == "auto":
            _state.whisper_asr_model = load_whisper_asr(
                model="openai/whisper-large-v3",
                base="openai/whisper-large-v3",
                language="auto",
                device_policy=bootstrap.device_policy,
            )
        else:
            _state.whisper_asr_model = load_whisper_asr(
                language=language,
                device_policy=bootstrap.device_policy,
            )
        _state.asr_engine = AsrEngine.WHISPER


def resolve_asr_engine(
    engine_overrides: dict[str, str] | None,
    rev_api_key: RevAiApiKey | None,
) -> str:
    """Resolve which ASR engine this worker should load.

    The precedence is intentionally simple and testable:

    1. explicit engine override from the Rust control plane
    2. Rev.AI when a key is available
    3. local Whisper fallback
    """
    if engine_overrides and "asr" in engine_overrides:
        return engine_overrides["asr"]
    return "rev" if rev_api_key else "whisper"


def resolve_injected_revai_api_key(
    environ: Mapping[str, str] | None = None,
) -> RevAiApiKey | None:
    """Resolve a pre-injected Rev.AI key from an explicit environment mapping."""
    env = environ if environ is not None else os.environ
    for key_name in ("BATCHALIGN_REV_API_KEY", "REVAI_API_KEY"):
        env_value = env.get(key_name)
        if env_value and env_value.strip():
            return env_value.strip()
    return None

__all__ = [
    "iso3_to_language_name",
    "load_asr_engine",
    "resolve_injected_revai_api_key",
    "resolve_asr_engine",
]
