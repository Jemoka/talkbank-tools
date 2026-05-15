"""Structural Protocol types for third-party objects used across inference.

All protocols are defined here to avoid runtime imports of heavy libraries
(stanza, torch, torchaudio).  They are only used for static type checking.

Usage::

    from __future__ import annotations
    from typing import TYPE_CHECKING

    if TYPE_CHECKING:
        from batchalign.inference.types import StanzaDoc, StanzaNLP
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, TypeAlias, runtime_checkable

import numpy as np

from batchalign.inference._domain_types import LanguageCode, SampleRate

if TYPE_CHECKING:
    import torch
    from transformers import (
        GenerationConfig,
        WhisperForConditionalGeneration,
        WhisperProcessor,
    )
    from transformers.pipelines import AutomaticSpeechRecognitionPipeline


# ---------------------------------------------------------------------------
# Stanza Protocols
# ---------------------------------------------------------------------------

class StanzaWord(Protocol):
    """Structural type for ``stanza.models.common.doc.Word``."""

    @property
    def text(self) -> str: ...

    @property
    def upos(self) -> str: ...

    @property
    def lemma(self) -> str: ...

    @property
    def feats(self) -> str | None: ...

    @property
    def deprel(self) -> str: ...

    @property
    def head(self) -> int: ...

    @property
    def id(self) -> int | tuple[int, ...]: ...


class StanzaToken(Protocol):
    """Structural type for ``stanza.models.common.doc.Token``."""

    @property
    def text(self) -> str: ...

    @property
    def id(self) -> tuple[int, ...]: ...

    @property
    def words(self) -> list[StanzaWord]: ...


class ConstituencyTree(Protocol):
    """Structural type for a constituency parse tree node."""

    @property
    def children(self) -> list[ConstituencyTree]: ...

    @property
    def label(self) -> str | None: ...

    def is_leaf(self) -> bool: ...


class StanzaSentence(Protocol):
    """Structural type for ``stanza.models.common.doc.Sentence``."""

    @property
    def tokens(self) -> list[StanzaToken]: ...

    @property
    def words(self) -> list[StanzaWord]: ...

    @property
    def constituency(self) -> ConstituencyTree: ...


class StanzaDoc(Protocol):
    """Structural type for ``stanza.models.common.doc.Document``."""

    @property
    def sentences(self) -> list[StanzaSentence]: ...

    def to_dict(self) -> list[list[dict[str, str | int | float | list[int] | tuple[int, ...] | None]]]:
        """Serialize the document to a nested list of word-level dicts."""
        ...


class StanzaNLP(Protocol):
    """Structural type for a Stanza Pipeline."""

    def __call__(self, text: str) -> StanzaDoc: ...


# ---------------------------------------------------------------------------
# Audio file Protocol
# ---------------------------------------------------------------------------

@runtime_checkable
class AudioFile(Protocol):
    """Structural type for ``ASRAudioFile``."""

    @property
    def file(self) -> str: ...

    @property
    def tensor(self) -> torch.Tensor: ...

    @property
    def rate(self) -> int: ...

    def file_identity(self) -> str: ...

    def chunk(self, begin_ms: int, end_ms: int) -> torch.Tensor: ...


# ---------------------------------------------------------------------------
# FA model return type aliases
# ---------------------------------------------------------------------------

WhisperFAResult = list[tuple[str, float]]
"""Whisper FA output: list of (token_text, timestamp_seconds)."""

Wave2VecFAResult = list[tuple[str, tuple[int, int]]]
"""Wave2Vec2 FA output: list of (word_text, (start_frame, end_frame))."""


# ---------------------------------------------------------------------------
# Typed model handles (replace monkey-patched object / tuple hacks)
# ---------------------------------------------------------------------------


"""Generation kwargs for Whisper ASR: repetition_penalty, config, task, language, max_new_tokens."""
GenerateKwargs: TypeAlias = "dict[str, str | int | float | GenerationConfig]"


class WhisperASRHandle:
    """Typed wrapper for a HuggingFace ASR pipeline with metadata.

    Replaces the monkey-patching pattern where config, lang, and sample_rate
    were stashed as ``_ba_*`` attributes on the pipeline object.
    """

    def __init__(
        self,
        pipe: AutomaticSpeechRecognitionPipeline,
        config: GenerationConfig,
        lang: LanguageCode,
        sample_rate: SampleRate,
        *,
        skip_language_force: bool = False,
    ) -> None:
        self._pipe = pipe
        self.config = config
        self.lang = lang
        self.sample_rate = sample_rate
        # HuggingFace Whisper fine-tunes pin language/task inside their
        # own ``generation_config``. Re-forcing those via ``generate_kwargs``
        # produces cross-script gibberish. When this flag is ``True``,
        # ``gen_kwargs`` omits ``task`` and ``language`` regardless of the
        # requested language — the caller is trusting the checkpoint's
        # own configuration. Used by the ``whisper_hub`` engine variant;
        # see ``python/batchalign/inference/whisper_hub.py``.
        self.skip_language_force = skip_language_force

    def __call__(
        self,
        audio: np.ndarray | str,  # mono waveform or provider-native input path
        *,
        batch_size: int = 1,
        generate_kwargs: GenerateKwargs | None = None,
    ) -> dict[str, list[dict[str, str | tuple[float, float]]]]:
        return self._pipe(  # type: ignore[no-any-return]
            audio,
            batch_size=batch_size,
            generate_kwargs=generate_kwargs or {},
        )

    def gen_kwargs(self, lang: LanguageCode) -> GenerateKwargs:
        """Build generation kwargs for a given language.

        Three modes:

        - ``skip_language_force=True`` (HF fine-tunes) → only
          ``max_new_tokens=444``. Fine-tunes bake their own
          ``generation_config`` / language hints / suppress-token
          set during training; overriding any of those degrades
          output. The ``max_new_tokens`` cap is a hard upper bound on
          per-chunk generation, not a decoding override: Whisper's
          ``max_target_positions=448`` includes the 3 special start
          tokens, so 444 is one below the legal max — a no-op under
          successful operation, a terminator for the intermittent
          runaway-generation case where the decoder fails to predict
          end-of-utterance and would otherwise spin indefinitely.

        - ``lang == "auto"`` (stock Whisper, multilingual) → omit
          ``language`` / ``task`` so Whisper auto-detects, but keep
          the stock-model tuning knobs.

        - Otherwise (stock Whisper, concrete language) → force the
          language and task on the stock checkpoint's decoder.
          Cantonese is a special case where even stock Whisper lacks
          a clean language hint; skip the forcing there too.
        """
        if self.skip_language_force:
            # See docstring — only the safety cap, no overrides.
            return {"max_new_tokens": 444}
        if lang == "auto":
            return {
                "repetition_penalty": 1.001,
                "generation_config": self.config,
            }
        kw: GenerateKwargs = {
            "repetition_penalty": 1.001,
            "generation_config": self.config,
            "task": "transcribe",
            "language": lang,
        }
        if lang == "Cantonese":
            kw = {"repetition_penalty": 1.001, "generation_config": self.config}
        return kw


class WhisperFAHandle:
    """Typed wrapper for Whisper forced alignment model bundle.

    Replaces the ``(model, processor, sample_rate)`` tuple with named fields.
    """

    def __init__(
        self,
        model: WhisperForConditionalGeneration,
        processor: WhisperProcessor,
        sample_rate: SampleRate,
    ) -> None:
        self.model = model
        self.processor = processor
        self.sample_rate = sample_rate


class Wave2VecFAHandle:
    """Typed wrapper for Wave2Vec forced alignment model bundle.

    Replaces the ``(model, sample_rate)`` tuple with named fields.
    """

    def __init__(
        self,
        model: torch.nn.Module,
        sample_rate: SampleRate,
    ) -> None:
        self.model = model
        self.sample_rate = sample_rate
