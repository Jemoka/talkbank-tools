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


__all__ = ["disable_torchcodec"]
