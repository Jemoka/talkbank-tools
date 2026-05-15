"""Operation handlers for worker metadata and capability reporting."""

from __future__ import annotations

import logging
import os
import time
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from batchalign.worker._model_loading.bootstrap import EnsureTaskResponse

from batchalign.worker._types import (
    CapabilitiesResponse,
    HealthResponse,
    InferTask,
    _state,
)

L = logging.getLogger("batchalign.worker")


def _health() -> HealthResponse:
    """Health check with worker metadata."""
    is_ready = _state.ready or _state.test_echo
    return HealthResponse(
        status="ok" if is_ready else "loading",
        command=_state.command,
        lang=_state.lang,
        pid=os.getpid(),
        uptime_s=time.monotonic() - _state.started_at,
    )


def _capabilities() -> CapabilitiesResponse:
    """Report available commands and runtime info.

    Command advertisement is intentionally narrower than infer-task
    advertisement. Server-owned compositions such as ``transcribe`` are not
    exposed as Python commands; Rust synthesizes them from lower-level
    capability signals.
    """
    if _state.test_echo:
        from batchalign.runtime import Cmd2Task

        commands = sorted(set(Cmd2Task.keys()) | {"test-echo"})
        # Advertise all infer tasks so the server's capability gate passes.
        # Echo workers handle any task by echoing back the payload.
        all_infer_tasks = list(InferTask)
        echo_versions = {task: "test-echo" for task in all_infer_tasks}
        return CapabilitiesResponse(
            commands=commands,
            free_threaded=False,
            infer_tasks=all_infer_tasks,
            engine_versions=echo_versions,
        )

    from batchalign.runtime import is_free_threaded

    infer_tasks: list[InferTask] = []
    engine_versions: dict[InferTask, str] = {}

    from batchalign.worker._stanza_capabilities import resolve_stanza_version

    # Infer task probes: map each InferTask to the imports required to prove
    # the system *can* run it.  The probe worker only loads morphotag models,
    # so we must NOT gate on loaded model state — otherwise FA, translate,
    # utseg, ASR, etc. are silently excluded from server capabilities.
    #
    # The default-version slot is left empty for stanza-backed tasks
    # (MORPHOSYNTAX / UTSEG / COREF) on purpose: those tasks always
    # resolve their version through `resolve_stanza_version()` below,
    # so the default is unused. Putting the literal engine name
    # ("stanza") here as a fallback would re-introduce the
    # `engine=stanza-stanza` corruption if the resolver were ever
    # bypassed by a future refactor.
    _INFER_TASK_PROBES: dict[InferTask, tuple[tuple[str, ...], str]] = {
        InferTask.MORPHOSYNTAX: (("stanza",), ""),
        InferTask.UTSEG:        (("stanza",), ""),
        InferTask.COREF:        (("stanza",), ""),
        InferTask.TRANSLATE:    (("googletrans",), "googletrans-v1"),
        InferTask.FA:           (("torch", "torchaudio"), "whisper"),
        InferTask.OPENSMILE:    (("opensmile",), "opensmile"),
        InferTask.AVQI:         (("parselmouth", "torchaudio"), "praat"),
    }

    import importlib

    def _module_importable(module_name: str) -> bool:
        try:
            importlib.import_module(module_name)
        except (ImportError, ModuleNotFoundError):
            return False
        return True

    for task, (deps, default_version) in _INFER_TASK_PROBES.items():
        importable = all(_module_importable(dep) for dep in deps)
        if importable:
            infer_tasks.append(task)
            if task == InferTask.MORPHOSYNTAX or task == InferTask.COREF:
                engine_versions[task] = resolve_stanza_version(_state.stanza_version)
            elif task == InferTask.UTSEG:
                engine_versions[task] = (
                    _state.utseg_version
                    or resolve_stanza_version(_state.stanza_version)
                )
            elif task == InferTask.FA:
                engine_versions[task] = _state.fa_model_name or _state.fa_engine.value
            elif task == InferTask.ASR:
                engine_versions[task] = _state.asr_engine.value
            elif task == InferTask.TRANSLATE:
                from batchalign.inference._domain_types import TranslationBackend
                if _state.translate_backend == TranslationBackend.GOOGLE:
                    engine_versions[task] = "googletrans-v1"
                else:
                    engine_versions[task] = default_version
            else:
                engine_versions[task] = default_version

    speaker_versions: list[str] = []
    if _module_importable("pyannote.audio"):
        speaker_versions.append("pyannote")
    if _module_importable("nemo.collections.asr"):
        speaker_versions.append("nemo")
    if speaker_versions:
        infer_tasks.append(InferTask.SPEAKER)
        engine_versions[InferTask.SPEAKER] = speaker_versions[0]

    # ASR is special now: the server can satisfy ASR through either
    # Python-hosted local engines (for example Whisper) or the Rust-owned
    # Rev.AI path when the control plane has already injected credentials.
    from batchalign.worker._model_loading.asr import resolve_injected_revai_api_key

    has_revai_key = bool(
        (_state.bootstrap and _state.bootstrap.revai_api_key)
        or resolve_injected_revai_api_key()
    )

    has_whisper = _module_importable("whisper")

    if has_whisper or has_revai_key:
        infer_tasks.append(InferTask.ASR)
        engine_versions[InferTask.ASR] = (
            _state.asr_engine.value
            if _state.ready and _state.asr_engine.value
            else "rev" if has_revai_key else "whisper"
        )

    # Build per-language Stanza capability map from resources.json.
    stanza_caps: dict[str, StanzaLanguageProcessors] = {}
    try:
        from batchalign.worker._stanza_capabilities import get_cached_capability_table
        from batchalign.worker._types import StanzaLanguageProcessors

        table = get_cached_capability_table()
        if table is not None:
            for iso3, caps in table.languages.items():
                processors = []
                if caps.has_tokenize:
                    processors.append("tokenize")
                if caps.has_pos:
                    processors.append("pos")
                if caps.has_lemma:
                    processors.append("lemma")
                if caps.has_depparse:
                    processors.append("depparse")
                if caps.has_mwt:
                    processors.append("mwt")
                if caps.has_constituency:
                    processors.append("constituency")
                if caps.has_coref:
                    processors.append("coref")
                stanza_caps[iso3] = StanzaLanguageProcessors(
                    alpha2=caps.alpha2,
                    processors=processors,
                )
    except Exception as e:
        L.warning("Failed to build stanza_capabilities: %s", e)

    return CapabilitiesResponse(
        commands=[],
        free_threaded=is_free_threaded(),
        infer_tasks=infer_tasks,
        engine_versions=engine_versions,
        stanza_capabilities=stanza_caps,
    )


# ---------------------------------------------------------------------------
# ensure_task — on-demand model loading for LazyProfile workers
# ---------------------------------------------------------------------------


def _ensure_task(
    task: str, engine_overrides: dict[str, str] | None
) -> EnsureTaskResponse:
    """Load one task's models on demand.

    Called by the Rust control plane before dispatching work to a LazyProfile
    worker. Idempotent: if the task is already loaded, returns immediately.
    Returns the Pydantic ``EnsureTaskResponse`` directly for JSON serialization.
    """
    from batchalign.worker._model_loading import ensure_task_loaded

    return ensure_task_loaded(task, engine_overrides)
