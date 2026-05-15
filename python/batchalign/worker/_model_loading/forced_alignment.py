"""Forced-alignment engine bootstrap helpers for worker startup."""

from __future__ import annotations

from batchalign.worker._types import FaEngine, WorkerBootstrapRuntime, _state


def load_fa_engine(bootstrap: WorkerBootstrapRuntime) -> None:
    """Load the forced-alignment engine for this worker."""
    lang = bootstrap.lang
    engine_overrides = bootstrap.engine_overrides or None
    fa_engine = engine_overrides["fa"] if engine_overrides and "fa" in engine_overrides else "whisper"
    if fa_engine == "wav2vec_canto":
        from batchalign.inference.languages.cantonese._cantonese_fa import load_cantonese_fa

        load_cantonese_fa(
            lang,
            engine_overrides,
            device_policy=bootstrap.device_policy,
        )
        _state.fa_engine = FaEngine.WAV2VEC_CANTO
        _state.fa_model_name = "wav2vec-canto-v1"
    elif fa_engine == "whisper":
        from batchalign.inference.fa import load_whisper_fa

        _state.whisper_fa_model = load_whisper_fa(
            device_policy=bootstrap.device_policy
        )
        _state.fa_engine = FaEngine.WHISPER
        _state.fa_model_name = "whisper-fa-large-v2"
    else:
        import importlib.metadata as importlib_metadata

        from batchalign.inference.fa import load_wave2vec_fa

        _state.wave2vec_fa_model = load_wave2vec_fa(
            device_policy=bootstrap.device_policy
        )
        _state.fa_engine = FaEngine.WAVE2VEC
        try:
            torchaudio_version = importlib_metadata.version("torchaudio")
            _state.fa_model_name = f"wave2vec-fa-mms-{torchaudio_version}"
        except importlib_metadata.PackageNotFoundError:
            _state.fa_model_name = "wave2vec-fa-mms"
