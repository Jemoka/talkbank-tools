"""Worker model-loading package.

This package keeps worker bootstrap concerns split by responsibility:

- `bootstrap.py` owns infer-task startup orchestration
- `translation.py` owns translation-engine bootstrap
- `forced_alignment.py` owns FA-engine bootstrap
- `asr.py` owns ASR-engine bootstrap

The import surface stays stable so the worker entrypoint can still depend on
`batchalign.worker._model_loading` as one module-level boundary.
"""

from batchalign.worker._model_loading.asr import (
    load_asr_engine,
    resolve_injected_revai_api_key,
)
from batchalign.worker._model_loading.bootstrap import (
    EnsureTaskResponse,
    enable_test_echo,
    ensure_task_loaded,
    load_worker_profile,
    load_worker_profile_lazy,
    load_worker_task,
    parse_engine_overrides,
)
from batchalign.worker._model_loading.forced_alignment import load_fa_engine
from batchalign.worker._model_loading.translation import load_translation_engine
from batchalign.worker._model_loading.utterance import load_utterance_model

__all__ = [
    "EnsureTaskResponse",
    "enable_test_echo",
    "ensure_task_loaded",
    "load_asr_engine",
    "load_fa_engine",
    "load_translation_engine",
    "load_utterance_model",
    "load_worker_profile",
    "load_worker_profile_lazy",
    "load_worker_task",
    "parse_engine_overrides",
    "resolve_injected_revai_api_key",
]
