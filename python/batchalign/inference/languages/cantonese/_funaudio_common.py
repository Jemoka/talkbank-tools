"""FunASR helpers for built-in HK/Cantonese engines."""

from __future__ import annotations

import io
import json
import logging
from contextlib import redirect_stdout
from dataclasses import dataclass, field
from typing import Any

from ._asr_types import AsrGenerationPayload, AsrMonologue, TimedWord
from batchalign.inference._domain_types import LanguageCode

L = logging.getLogger("batchalign.hk.funaudio")


@dataclass
class FunAsrSegment:
    """Parsed output from one FunASR model segment.

    FunASR returns raw dicts — this type captures the fields we use and
    validates at the boundary so downstream code never touches raw dicts.
    """

    text: str
    timestamp: list[list[int | float]] = field(default_factory=list)

    @classmethod
    def from_raw(cls, raw: dict[str, Any]) -> FunAsrSegment:
        """Parse a raw FunASR segment dict into a typed object."""
        text = str(raw.get("text", ""))
        timestamps = raw.get("timestamp")
        if not isinstance(timestamps, list):
            timestamps = []
        return cls(text=text, timestamp=timestamps)


class FunAudioRecognizer:
    """Wrapper around FunASR model invocation plus Rust-owned projection."""

    def __init__(self, lang: LanguageCode = "yue", model: str = "FunAudioLLM/SenseVoiceSmall", device: str = "cpu") -> None:
        """Store language/model configuration and defer model loading until first use."""
        self.lang = lang
        self.model_name = model
        self.device = device
        self._model: Any | None = None

    def _get_model(self) -> Any:
        """Return a cached FunASR model instance, creating it on first call."""
        if self._model is not None:
            return self._model

        try:
            with redirect_stdout(io.StringIO()):
                from funasr import AutoModel
        except Exception as exc:
            raise ImportError(
                "FunAudio engine dependency 'funasr' is missing from this "
                "environment. Reinstall batchalign3 or install funasr."
            ) from exc

        with redirect_stdout(io.StringIO()):
            if "paraformer" not in self.model_name:
                self._model = AutoModel(
                    model=self.model_name,
                    output_timestamps=True,
                    vad_model="fsmn-vad",
                    vad_kwargs={"max_single_segment_time": 30000},
                    device=self.device,
                    hub="hf",
                    cache={},
                    language=self.lang,
                    use_itn=True,
                    batch_size_s=60,
                    output_timestamp=True,
                    ban_emo_unk=False,
                    merge_vad=True,
                    merge_length_s=15,
                )
            else:
                self._model = AutoModel(
                    model=self.model_name,
                    model_revision="v2.0.4",
                    vad_model="fsmn-vad",
                    vad_model_revision="v2.0.4",
                    punc_model="ct-punc-c",
                    punc_model_revision="v2.0.4",
                )
        return self._model

    @staticmethod
    def _clean_segment_text(text: str) -> str:
        """Delegate FunASR text cleanup to the shared Rust helper."""
        import batchalign_core

        return batchalign_core.clean_funaudio_segment_text(text)

    def _run_model(self, source_path: str) -> list[FunAsrSegment]:
        """Invoke FunASR and parse output into typed segments."""
        model = self._get_model()
        with redirect_stdout(io.StringIO()):
            if "paraformer" in self.model_name:
                output = model.generate(input=source_path, output_timestamp=True)
            else:
                output = model.generate(
                    input=source_path,
                    cache={},
                    language=self.lang,
                    output_timestamps=True,
                    vad_model="fsmn-vad",
                    vad_kwargs={"max_single_segment_time": 60000},
                    ban_emo_unk=False,
                    use_itn=True,
                    batch_size_s=60,
                    merge_vad=True,
                    merge_length_s=15,
                    output_timestamp=True,
                    spk_model="cam++",
                )

        raw_list: list[dict[str, Any]]
        if isinstance(output, dict):
            raw_list = [output]
        elif isinstance(output, list):
            raw_list = [item for item in output if isinstance(item, dict)]
        else:
            raw_list = []

        return [FunAsrSegment.from_raw(raw) for raw in raw_list]

    def transcribe(self, source_path: str) -> tuple[AsrGenerationPayload, list[TimedWord]]:
        """Return `(monologues_payload, timed_words)` for the source audio.

        The Python side only owns model invocation and shallow parsing. Rust
        owns token cleanup, Cantonese tokenization, timestamp pairing, and the
        projection into the shared ASR worker payload shape.
        """
        import batchalign_core

        segments = self._run_model(source_path)
        projection = json.loads(
            batchalign_core.funaudio_segments_to_asr(
                [
                    {"text": segment.text, "timestamp": segment.timestamp}
                    for segment in segments
                ],
                self.lang,
            )
        )
        monologues: list[AsrMonologue] = projection["monologues"]
        timed_words: list[TimedWord] = projection["timed_words"]
        return {"monologues": monologues}, timed_words
