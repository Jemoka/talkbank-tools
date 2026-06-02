"""Qwen2-Audio / Qwen2.5-Audio-Instruct ASR backend.

Two execution modes:

* **Local** (default): load the model via ``transformers``
  (``AutoProcessor`` + ``AutoModelForCausalLM``), build a chat prompt
  with the audio attached, generate, and decode.
* **Remote vLLM**: if ``engine.qwen.base_url`` is configured in
  ``~/.batchalign.ini`` the backend delegates to
  :class:`~batchalign.backends.asr.vllm.VllmAsrBackend`.

Qwen2-Audio does not natively produce word-level timestamps. We emit a
single :class:`AsrSegment` covering the full audio with the
transcription and no per-word timings; a downstream FA backend (e.g.
:class:`WhisperXBackend`, :class:`Wav2Vec2FaBackend`) supplies timing.
"""

from __future__ import annotations

import os
import tempfile
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy
from batchalign import config
from batchalign.lang import LanguageCode


_DEFAULT_PROMPT = "Transcribe the speech."


class QwenAsrBackend(ASR):
    """Qwen2-Audio chat-style ASR (no native word timestamps)."""

    _PROVIDER = "qwen"

    def __init__(
        self,
        model: str | None = None,
        *,
        language: LanguageCode,
        device: str | None = None,
        prompt: str = _DEFAULT_PROMPT,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        creds = config.get_provider(self._PROVIDER, interactive=True) if hasattr(config, "get_provider") else {}
        base_url = creds.get("base_url", "")
        configured_model = creds.get("model") or model or "Qwen/Qwen2-Audio-7B-Instruct"

        self._prompt = prompt
        self._model_id = configured_model
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)
        self._delegate: Any | None = None

        if base_url:
            # Delegate to vLLM-hosted Qwen.
            from batchalign.backends.asr.vllm import VllmAsrBackend

            self._delegate = VllmAsrBackend(
                model=configured_model,
                language=language,
                base_url=base_url,
                api_key=creds.get("api_key", "EMPTY"),
            )
            self._processor = None
            self._model = None
            self._device = None
            return

        from transformers import AutoProcessor, AutoModelForCausalLM  # type: ignore[import-not-found]

        if device is None:
            try:
                import torch  # type: ignore[import-not-found]

                device = "cuda" if torch.cuda.is_available() else "cpu"
            except ImportError:
                device = "cpu"
        self._processor = AutoProcessor.from_pretrained(configured_model)
        self._model = AutoModelForCausalLM.from_pretrained(configured_model).to(device)
        self._device = device

    @property
    def name(self) -> str:
        return f"qwen-asr:{self._model_id}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        if self._delegate is not None:
            return self._delegate.call(batch)

        from batchalign._core.proto import AsrInput, AsrOutput, AsrSegment
        from batchalign.backends.asr.vllm import pcm_to_wav_bytes

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(
                    f"QwenAsrBackend does not handle: {type(item).__name__}"
                )
            wav_bytes = pcm_to_wav_bytes(item.audio)
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                tmp.write(wav_bytes)
                tmp_path = tmp.name
            try:
                text = self._transcribe_one(tmp_path)
            finally:
                try:
                    os.unlink(tmp_path)
                except OSError:
                    pass

            # No native word timestamps; emit a single full-audio segment.
            duration_ms = int(
                (len(item.audio.pcm_f32le) / 4) / item.audio.sample_rate * 1000
            )
            if not text:
                outputs.append(AsrOutput(source_id=item.source_id, segments=[]))
                continue
            seg = AsrSegment(
                start_ms=0,
                end_ms=duration_ms,
                text=text,
                speaker=None,
                words=[],
            )
            outputs.append(AsrOutput(source_id=item.source_id, segments=[seg]))
        return outputs

    def _transcribe_one(self, wav_path: str) -> str:
        import librosa  # type: ignore[import-not-found]

        conversation = [
            {
                "role": "user",
                "content": [
                    {"type": "audio", "audio_url": wav_path},
                    {"type": "text", "text": self._prompt},
                ],
            }
        ]
        prompt = self._processor.apply_chat_template(
            conversation, add_generation_prompt=True, tokenize=False
        )
        audio_array, _ = librosa.load(
            wav_path, sr=self._processor.feature_extractor.sampling_rate
        )
        inputs = self._processor(
            text=prompt, audios=audio_array, return_tensors="pt", padding=True
        ).to(self._device)
        gen_ids = self._model.generate(**inputs, max_new_tokens=512)
        gen_ids = gen_ids[:, inputs.input_ids.size(1) :]
        decoded = self._processor.batch_decode(
            gen_ids, skip_special_tokens=True, clean_up_tokenization_spaces=False
        )
        return (decoded[0] if decoded else "").strip()


__all__ = ["QwenAsrBackend"]
