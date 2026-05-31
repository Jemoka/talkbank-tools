"""FunAudioBackend: FunASR ASR (SenseVoiceSmall / paraformer-zh) — BA2 parity.

Faithful port of BA2's `FunAudioEngine`
(`batchalign2/batchalign/pipelines/asr/funaudio.py`). Wraps `funasr.AutoModel`
in two configurations:

* ``FunAudioLLM/SenseVoiceSmall`` (default) — multilingual SenseVoice with VAD +
  inverse-text-normalization, pinned to Cantonese (``yue``). Its decoded text
  carries ``<|yue|>`` / ``<|withitn|>`` markers; we strip those, OpenCC-convert
  Simplified→HK-Traditional (``s2hk``), apply BA2's Cantonese word fixups, drop
  CJK punctuation, and emit CHARACTER-level items (Cantonese is char-segmented).
* ``funasr/paraformer-zh`` — Mandarin Paraformer (+ VAD + punctuation); emits
  whitespace-split word items.

It is a normal `ASR` backend: it produces `AsrSegment`s (one per FunASR/VAD
segment), and the transcribe recipe's CHATUtterance UtSeg pairing (the Cantonese
BERT for ``yue``) carves them into utterances — exactly as BA2 pairs its BERT
segmenter to the FunAudio output via `process_generation`.
"""

from __future__ import annotations

import re
import tempfile
from pathlib import Path
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy
from batchalign.lang import LanguageCode

# BA2's Cantonese surface-form fixups (funaudio.py:replace_cantonese_words).
_CANTONESE_WORD_REPLACEMENTS = {
    "系": "係", "繫": "係", "聯係": "聯繫", "系啊": "係啊", "真系": "真係",
    "唔系": "唔係", "呀": "啊", "噶": "㗎", "咧": "呢", "嗬": "喎", "只": "隻",
    "咯": "囉", "嚇": "吓", "飲": "飲", "喐": "郁", "食": "食", "啫": "咋",
    "哇": "嘩", "着": "著", "中意": "鍾意", "嘞": "喇", "啵": "噃", "遊水": "游水",
    "羣組": "群組", "古仔": "故仔", "甕": "㧬", "牀": "床", "松": "鬆",
    "較剪": "鉸剪", "吵": "嘈", "衝涼": "沖涼", "分鍾": "分鐘", "重復": "重複",
}
# Longest-first so multi-char keys win (BA2 sorts by length desc).
_CANTO_PATTERN = re.compile(
    "|".join(
        re.escape(k)
        for k in sorted(_CANTONESE_WORD_REPLACEMENTS, key=len, reverse=True)
    )
)
# CJK punctuation BA2 drops from the Cantonese text before char-segmenting.
_CJK_PUNCT_DROP = "「」。，！？"


class FunAudioBackend(ASR):
    """FunASR SenseVoice / Paraformer ASR (BA2 `FunAudioEngine`)."""

    def __init__(
        self,
        *,
        model: str = "FunAudioLLM/SenseVoiceSmall",
        language: LanguageCode,
        device: str | None = None,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from funasr import AutoModel  # type: ignore[import-not-found]

        # FunASR's `language=` kwarg takes 3-letter codes directly
        # (`yue`, `eng`, `cmn`, …); BA2 ground truth:
        # `batchalign2/batchalign/pipelines/asr/funaudio.py:65,146`.
        self._model_id = model
        self._lang = language.alpha_3
        self._is_paraformer = "paraformer" in model
        dev = device or "cpu"
        # Mirror BA2's AutoModel construction exactly for each model family.
        if not self._is_paraformer:
            self._model = AutoModel(
                model=model,
                output_timestamps=True,
                vad_model="fsmn-vad",
                vad_kwargs={"max_single_segment_time": 30000},
                device=dev,
                hub="hf",
                cache={},
                language="yue",
                use_itn=True,
                batch_size_s=60,
                output_timestamp=True,
                ban_emo_unk=False,
                merge_vad=True,
                merge_length_s=15,
            )
        else:
            self._model = AutoModel(
                model=model,
                model_revision="v2.0.4",
                vad_model="fsmn-vad",
                vad_model_revision="v2.0.4",
                punc_model="ct-punc-c",
                punc_model_revision="v2.0.4",
            )
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"funaudio:{self._model_id}:{self._lang}:v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    @staticmethod
    def _replace_cantonese(text: str) -> str:
        return _CANTO_PATTERN.sub(
            lambda m: _CANTONESE_WORD_REPLACEMENTS.get(m.group(0), m.group(0)), text
        )

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(f"FunAudioBackend does not handle: {type(item).__name__}")
            outputs.append(self._transcribe(item, AsrOutput))
        return outputs

    def _transcribe(self, item: Any, AsrOutput: type) -> Any:
        import numpy as np  # type: ignore[import-not-found]
        import soundfile as sf  # type: ignore[import-not-found]
        from opencc import OpenCC  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        cc = OpenCC("s2hk")
        wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
        sr = int(item.audio.sample_rate)

        # FunASR's AutoModel.generate wants a file path; write the decoded PCM
        # to a temp WAV (16 kHz mono float32 already prepared upstream).
        with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
            tmp_path = tmp.name
        try:
            sf.write(tmp_path, wave, sr)
            if not self._is_paraformer:
                res = self._model.generate(
                    input=tmp_path,
                    cache={},
                    language=self._lang,
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
            else:
                res = self._model.generate(input=tmp_path, output_timestamp=True)
        finally:
            try:
                Path(tmp_path).unlink()
            except OSError:
                pass

        segments_out: list[Any] = []
        for segment in res:
            text = segment.get("text", "")
            timestamps = segment.get("timestamp") or []

            # SenseVoice text carries <|yue|> / <|withitn|> markers; pull the
            # content out (BA2 funaudio.py:generate).
            current: list[str] = []
            for part in text.split("<|yue|>"):
                if not part.strip():
                    continue
                parts = part.strip().split("<|withitn|>", 1)
                if len(parts) > 1:
                    current.append(parts[1].strip())
                elif self._is_paraformer:
                    current.append(parts[0])
            large_string = "".join(current)
            if not large_string.strip():
                continue

            if self._lang == "yue" and not self._is_paraformer:
                content = cc.convert(large_string)
                content = self._replace_cantonese(content)
                for p in _CJK_PUNCT_DROP:
                    content = content.replace(p, "")
                items = list(content)  # char-level for Cantonese
            else:
                items = large_string.split()
            # BA2 parity: `process_generation` always feeds the segmenter
            # `" ".join(values)`, regardless of language. Mirror that — the
            # Cantonese BERT's char-level tokenization is space-sensitive.
            joiner = " "

            words: list[Any] = []
            for index, tok in enumerate(items):
                try:
                    s, e = timestamps[index]
                    words.append(
                        AsrWord(text=tok, start_ms=int(s), end_ms=int(e), confidence=None)
                    )
                except (IndexError, TypeError, ValueError):
                    words.append(AsrWord(text=tok, start_ms=0, end_ms=0, confidence=None))

            blob = joiner.join(items).strip()
            if not blob:
                continue
            timed = [w for w in words if w.end_ms > 0]
            segments_out.append(
                AsrSegment(
                    start_ms=timed[0].start_ms if timed else 0,
                    end_ms=timed[-1].end_ms if timed else 0,
                    text=blob,
                    speaker=None,
                    words=words,
                )
            )

        return AsrOutput(source_id=item.source_id, segments=segments_out)


__all__ = ["FunAudioBackend"]
