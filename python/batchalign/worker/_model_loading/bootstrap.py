"""Top-level worker bootstrap orchestration.

This module is the narrow control-plane entry for worker startup. Workers are
bootstrapped for one infer task at a time; Rust owns any higher-level command
composition.
"""

from __future__ import annotations

import json
import logging
import os
import time

from pydantic import BaseModel

from batchalign.inference._domain_types import LanguageCode
from batchalign.worker._infer_hosts import (
    build_morphosyntax_batch_infer_handler,
    build_translate_batch_infer_handler,
    build_utseg_batch_infer_handler,
)
from batchalign.worker._model_loading.asr import load_asr_engine
from batchalign.worker._model_loading.forced_alignment import load_fa_engine
from batchalign.worker._model_loading.translation import load_translation_engine
from batchalign.worker._model_loading.utterance import load_utterance_model
from batchalign.worker._progress import emit_download_event
from batchalign.worker._stanza_loading import load_stanza_models, load_utseg_builder
from batchalign.worker._types import (
    PROFILE_TASKS,
    InferTask,
    WorkerBootstrapRuntime,
    WorkerProfile,
    _state,
)

L = logging.getLogger("batchalign.worker")


def _configure_loaded_tasks(
    tasks: set[str],
    bootstrap: WorkerBootstrapRuntime,
    *,
    target_label: str,
) -> None:
    """Load one explicit task set and register the resulting handlers.

    Per-task elapsed_ms is emitted as `progress_v2` events so
    occasional slow bootstrap loads surface in the daemon log live.
    """
    lang = bootstrap.lang
    num_speakers = bootstrap.num_speakers
    L.info(
        "Loading models: target=%s lang=%s num_speakers=%d pid=%d",
        target_label,
        lang,
        num_speakers,
        os.getpid(),
    )
    _state.clear_batch_infer_handlers()
    # Sort tasks deterministically so log order across runs is
    # comparable; without this, set iteration order varies and
    # cross-run elapsed_ms outlier diffs are harder to spot.
    for task in sorted(tasks):
        task_start = time.monotonic()
        emit_download_event(
            stage=f"loading_task_{task}_{lang}_start",
            user_message=f"Loading {task} ({lang})…",
        )
        _load_single_task(task, bootstrap)
        elapsed_ms = int((time.monotonic() - task_start) * 1000)
        emit_download_event(
            stage=f"loading_task_{task}_{lang}_complete",
            user_message=f"Loaded {task} ({lang}) in {elapsed_ms} ms",
        )

    _state.loaded_tasks = set(tasks)
    _state.command = target_label
    _state.lang = lang
    _state.num_speakers = num_speakers
    _state.ready = True
    L.info("Models ready: target=%s pid=%d", target_label, os.getpid())

def load_worker_profile(bootstrap: WorkerBootstrapRuntime) -> None:
    """Load the ML/runtime state for a profile-based worker.

    Profile-based workers load models for all tasks in the profile, enabling
    model sharing within a single process. GPU-only models (speaker, opensmile,
    avqi) use lazy loading on first request — only ASR/FA/Stanza/translation
    models are loaded eagerly here.
    """
    if bootstrap.profile is None:
        raise ValueError("worker bootstrap runtime requires a profile")

    profile = bootstrap.profile
    tasks = PROFILE_TASKS[profile]
    _state.bootstrap = bootstrap
    _configure_loaded_tasks(
        tasks,
        bootstrap,
        target_label=f"profile:{profile.value}",
    )


def load_worker_task(bootstrap: WorkerBootstrapRuntime) -> None:
    """Load the ML/runtime state needed for one pure infer-task worker."""
    if bootstrap.task is None:
        raise ValueError("worker bootstrap runtime requires a task")

    task = bootstrap.task
    task_name = task.value
    _state.bootstrap = bootstrap
    _configure_loaded_tasks(
        {task_name},
        bootstrap,
        target_label=f"infer:{task_name}",
    )


def load_worker_profile_lazy(bootstrap: WorkerBootstrapRuntime) -> None:
    """Start a profile-based worker WITHOUT loading any models.

    The worker signals ready immediately (~500 MB resident). Models are loaded
    on demand via ``ensure_task_loaded()`` when the Rust control plane sends
    ``ensure_task`` IPC messages before dispatching work.

    Used on Medium-tier machines (24-48 GB) where eager profile loading would
    consume 10-15 GB of RAM speculatively. See ``LazyProfile`` bootstrap mode.
    """
    if bootstrap.profile is None:
        raise ValueError("worker bootstrap runtime requires a profile")

    _state.bootstrap = bootstrap
    _state.command = f"lazy-profile:{bootstrap.profile.value}"
    _state.lang = bootstrap.lang
    _state.num_speakers = bootstrap.num_speakers
    _state.ready = True
    L.info(
        "Lazy profile ready (no models loaded): target=%s lang=%s pid=%d",
        _state.command,
        _state.lang,
        os.getpid(),
    )


class EnsureTaskResponse(BaseModel):
    """Response for the ``ensure_task`` IPC operation.

    Returned by :func:`ensure_task_loaded` and serialized directly to JSON
    over the worker protocol.
    """

    status: str
    task: str
    elapsed_s: float


# Valid task names for ensure_task. Includes InferTask values plus the
# legacy "utterance" alias that _load_single_task maps to "utseg".
_VALID_ENSURE_TASKS: frozenset[str] = frozenset(
    {t.value for t in InferTask} | {"utterance"}
)


def ensure_task_loaded(
    task: str,
    engine_overrides: dict[str, str] | None = None,
) -> EnsureTaskResponse:
    """Load one task's models on demand. Idempotent and thread-safe.

    If the task is already loaded, returns immediately with
    ``status="already_loaded"``. Otherwise loads the task's models under a lock
    to prevent concurrent duplicate loads.

    The ``engine_overrides`` parameter lets the Rust control plane specify which
    engine variant to load (e.g., ``{"fa": "wave2vec"}``). When ``None``, the
    worker falls back to its bootstrap-time overrides.

    Raises ``ValueError`` if *task* is not a recognized ``InferTask`` value
    (or the legacy ``"utterance"`` alias).
    """
    import time as _time

    if task not in _VALID_ENSURE_TASKS:
        raise ValueError(
            f"Unknown ensure_task name {task!r}; "
            f"valid names: {sorted(_VALID_ENSURE_TASKS)}"
        )

    if task in _state.loaded_tasks:
        return EnsureTaskResponse(status="already_loaded", task=task, elapsed_s=0.0)

    with _state.loading_lock:
        # Double-check after acquiring lock (another thread may have loaded it).
        if task in _state.loaded_tasks:
            return EnsureTaskResponse(status="already_loaded", task=task, elapsed_s=0.0)

        bootstrap = _state.bootstrap
        if bootstrap is None:
            raise RuntimeError("ensure_task_loaded called before worker bootstrap")

        merged_overrides = dict(bootstrap.engine_overrides) if bootstrap.engine_overrides else {}
        if engine_overrides:
            merged_overrides.update(engine_overrides)

        task_bootstrap = WorkerBootstrapRuntime(
            task=bootstrap.task,
            profile=bootstrap.profile,
            lang=bootstrap.lang,
            num_speakers=bootstrap.num_speakers,
            engine_overrides=merged_overrides,
            device_policy=bootstrap.device_policy,
            revai_api_key=bootstrap.revai_api_key,
        )

        t0 = _time.monotonic()
        L.info("Loading task on demand: task=%s engine_overrides=%s pid=%d",
               task, merged_overrides, os.getpid())
        _load_single_task(task, task_bootstrap)

        elapsed = _time.monotonic() - t0
        _state.loaded_tasks.add(task)
        L.info("Task loaded: task=%s elapsed=%.1fs pid=%d", task, elapsed, os.getpid())
        return EnsureTaskResponse(status="loaded", task=task, elapsed_s=elapsed)


def _load_single_task(task: str, bootstrap: WorkerBootstrapRuntime) -> None:
    """Load one task's models and register its handlers.

    Reuses the same loading functions as ``_configure_loaded_tasks()`` but for
    a single task at a time. This is the core of on-demand loading.

    Alias mappings:
    - ``"coref"`` → loads Stanza (same as ``"morphosyntax"``), registers
      the morphosyntax handler (coref shares Stanza models).
    - ``"utterance"`` → loads utseg models (legacy alias from PROFILE_TASKS).
    - ``"speaker"``, ``"opensmile"``, ``"avqi"`` → no-ops here; these use
      lazy loading at request time inside their execute_v2 handlers.
    """
    engine_overrides = bootstrap.engine_overrides or None

    if task in (InferTask.MORPHOSYNTAX.value, InferTask.COREF.value):
        # The IPC wire format is `--lang STRING`, so Python sees a plain
        # string. The Rust side may pass the typed `WorkerLanguage::Auto`
        # sentinel (serialized as "auto") for capability-probe spawns —
        # those workers must not eagerly load Stanza for an ISO-code that
        # doesn't exist in Stanza's catalog. Skip the model load and let
        # the per-file dispatch path load language-specific Stanza pipelines
        # later via `ensure_task`. Pre-existing pre-PerFile behavior treated
        # any string as a real ISO code; that crashed on "auto" with
        # UnsupportedLanguageError before the worker emitted a ready signal.
        if bootstrap.lang in ("auto", "per-file"):
            L.info(
                "Skipping eager Stanza load for non-ISO lang '%s'; "
                "language-specific pipelines will be loaded per-file",
                bootstrap.lang,
            )
        else:
            load_stanza_models(bootstrap.lang)
        _state.register_batch_infer_handler(
            InferTask.MORPHOSYNTAX,
            build_morphosyntax_batch_infer_handler(),
        )
    elif task in ("utterance", InferTask.UTSEG.value):
        if bootstrap.lang in ("auto", "per-file"):
            L.info(
                "Skipping eager utseg-model load for non-ISO lang '%s'; "
                "models will be loaded on first request",
                bootstrap.lang,
            )
        else:
            load_utseg_builder(bootstrap.lang)
            load_utterance_model(bootstrap.lang)
        _state.register_batch_infer_handler(
            InferTask.UTSEG,
            build_utseg_batch_infer_handler(),
        )
    elif task == InferTask.TRANSLATE.value:
        load_translation_engine(engine_overrides)
        _state.register_batch_infer_handler(
            InferTask.TRANSLATE,
            build_translate_batch_infer_handler(),
        )
    elif task == InferTask.FA.value:
        load_fa_engine(bootstrap)
    elif task == InferTask.ASR.value:
        load_asr_engine(bootstrap)
    elif task in (InferTask.SPEAKER.value, InferTask.OPENSMILE.value, InferTask.AVQI.value):
        # These tasks use lazy loading at request time — no startup model load.
        pass
    else:
        raise ValueError(f"Unknown task for on-demand loading: {task!r}")


def enable_test_echo(target_label: str, lang: LanguageCode) -> None:
    """Configure the worker to echo requests without loading models."""
    _state.test_echo = True
    _state.clear_batch_infer_handlers()
    _state.command = target_label or "test-echo"
    _state.lang = lang
    _state.ready = True


def parse_engine_overrides(raw: str) -> dict[str, str] | None:
    """Parse the engine-override JSON payload from CLI args.

    The Rust caller serializes a ``BTreeMap<String, String>``; we validate
    the shape here at the IPC boundary.
    """
    if not raw:
        return None
    parsed: object = json.loads(raw)
    if not isinstance(parsed, dict) or not all(
        isinstance(k, str) and isinstance(v, str) for k, v in parsed.items()
    ):
        raise ValueError(
            f"engine overrides must be a flat {{str: str}} object, got: {raw!r}"
        )
    return parsed
