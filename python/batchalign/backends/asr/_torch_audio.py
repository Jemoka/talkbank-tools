"""Work around a broken `torchcodec` in the transformers ASR pipeline.

`transformers >= 5` calls `import torchcodec` inside the ASR pipeline's
`preprocess` whenever `is_torchcodec_available()` is true — even for already-
decoded array input. In some environments (e.g. the Bazel-hermetic
`torch 2.10` + `torchcodec` + FFmpeg combo on macOS) the `torchcodec` shared
library fails to load, so that line raises `RuntimeError: Could not load
libtorchcodec` and ASR never runs.

We feed the pipeline a pre-decoded PCM numpy array (the Rust side already
decoded the media via `prepare_pcm`), so the `torchcodec` branch is dead code
for us. Forcing `is_torchcodec_available()` to report `False` makes the
pipeline skip the import and take the array path (resampling, if any, goes
through `torchaudio`, which loads fine).

This is import-path surgery on a third-party library, scoped to the ASR
backends; it does not affect any code that genuinely wants torchcodec.
"""

from __future__ import annotations

from typing import Any


def disable_torchcodec() -> None:
    """Best-effort: make transformers' ASR pipeline skip the torchcodec import.

    Idempotent and silent if transformers isn't importable or already lacks
    the symbol. Patch the name where the ASR pipeline module bound it (it does
    `from ..utils import is_torchcodec_available`), plus the utils source.
    """
    try:
        import transformers.pipelines.automatic_speech_recognition as asr_mod  # type: ignore[import-not-found]

        if hasattr(asr_mod, "is_torchcodec_available"):
            asr_mod.is_torchcodec_available = lambda: False
    except Exception:
        pass
    try:
        import transformers.utils as tfm_utils  # type: ignore[import-not-found]

        if hasattr(tfm_utils, "is_torchcodec_available"):
            tfm_utils.is_torchcodec_available = lambda: False
    except Exception:
        pass


def ctc_timestamp_scale(chunks: list[Any], *, duration_s: float) -> float:
    """Return a correction for clear integer-multiple CTC timestamp drift.

    Some older checkpoints expose an output stride that disagrees with the
    value Transformers uses to convert token offsets to seconds. Use physical
    PCM duration as a model-independent sanity bound, correcting only a close
    2x, 3x, or 4x multiple. Normal timestamps are returned unchanged.
    """
    ends = [
        float(ts[1])
        for chunk in chunks
        if (ts := chunk.get("timestamp")) and ts[1] is not None
    ]
    if not ends or duration_s <= 0:
        return 1.0
    ratio = max(ends) / duration_s
    multiple = round(ratio)
    if 2 <= multiple <= 4 and abs(ratio - multiple) <= 0.2:
        return 1.0 / multiple
    return 1.0


__all__ = ["ctc_timestamp_scale", "disable_torchcodec"]
