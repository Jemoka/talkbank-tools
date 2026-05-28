"""TencentAsrBackend: Tencent Cloud ASR — BA2 parity.

Faithful port of BA2's `TencentEngine`
(`batchalign2/batchalign/pipelines/asr/tencent.py`). Uploads the media to a
Tencent Cloud COS bucket, creates an async recognition task
(`16k_zh_large` for zh/yue/wuu/nan/hak, else `16k_<lang>`) with speaker
diarization, polls to completion, then projects the word-level diarized result
into `AsrSegment`s (one per `ResultDetail` speaker turn). For Cantonese (`yue`)
each word is OpenCC-converted (`s2hk`) and run through BA2's word fixups.

Credentials (`id` / `key` / `region` / `bucket`) come from `~/.batchalign.ini`
under `[asr] engine.tencent.*`, read via `batchalign.config.get_provider`.

It is a normal `ASR` backend: the transcribe recipe's CHATUtterance UtSeg
pairing carves the turns into utterances (the Cantonese BERT for `yue`), exactly
as BA2 pairs its BERT segmenter to the Tencent output via `process_generation`.
"""

from __future__ import annotations

import pathlib
import time
import uuid
from typing import Any

from batchalign.backends.base import ASR, BatchPolicy
from batchalign import config

# BA2's Cantonese surface-form fixups (tencent.py:replace_cantonese_words).
_CANTONESE_WORD_REPLACEMENTS = {
    "系": "係", "唔系": "唔係", "噶": "㗎", "咧": "呢", "嗬": "喎", "只": "隻",
    "咯": "囉", "嚇": "吓", "飲": "飲", "喐": "郁", "食": "食", "啫": "咋",
    "哇": "嘩", "着": "著", "中意": "鍾意", "嘞": "喇", "啵": "噃", "遊水": "游水",
    "羣組": "群組", "古仔": "故仔", "甕": "㧬", "牀": "床", "松": "鬆",
    "較剪": "鉸剪", "吵": "嘈", "衝涼": "沖涼", "分鍾": "分鐘", "重復": "重複",
}
# Tencent's large Chinese model covers these (BA2 tencent.py).
_ZH_LARGE_LANGS = {"zho", "yue", "wuu", "nan", "hak"}
_POLL_INTERVAL_S = 15
_DONE_STATUS = {2, 3}
_FAILED_STATUS = {3, "3"}


class TencentAsrBackend(ASR):
    """Tencent Cloud ASR (BA2 `TencentEngine`) — COS upload + diarized async ASR."""

    def __init__(
        self,
        *,
        lang: str = "eng",
        num_speakers: int = 2,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        from tencentcloud.common.credential import Credential  # type: ignore[import-not-found]
        from tencentcloud.asr.v20190614.asr_client import AsrClient  # type: ignore[import-not-found]
        from qcloud_cos import CosConfig, CosS3Client  # type: ignore[import-not-found]

        creds = config.get_provider("tencent")
        try:
            self._id = creds["id"]
            self._key = creds["key"]
            self._region = creds["region"]
            self._bucket = creds["bucket"]
        except KeyError as e:
            raise RuntimeError(
                "Tencent Cloud not configured: set engine.tencent.{id,key,region,bucket} "
                f"under [asr] in ~/.batchalign.ini (missing {e})."
            ) from e

        self._lang_code = lang
        self._num_speakers = num_speakers
        if lang == "yue":
            self._lang = "yue"
        else:
            import pycountry  # type: ignore[import-not-found]

            rec = pycountry.languages.get(alpha_3=lang)
            self._lang = rec.alpha_2 if rec is not None else lang

        cos_config = CosConfig(
            Region=self._region, SecretId=self._id, SecretKey=self._key,
            Token=None, Scheme="https",
        )
        self._cos = CosS3Client(cos_config)
        self._client = AsrClient(Credential(self._id, self._key), "ap-hongkong")
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"tencent:{self._lang_code}:async-v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    @staticmethod
    def _replace_cantonese(word: str) -> str:
        return _CANTONESE_WORD_REPLACEMENTS.get(word, word)

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import AsrInput, AsrOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AsrInput):
                raise TypeError(f"TencentAsrBackend does not handle: {type(item).__name__}")
            outputs.append(self._transcribe(item, AsrOutput))
        return outputs

    def _transcribe(self, item: Any, AsrOutput: type) -> Any:
        import tempfile
        import numpy as np  # type: ignore[import-not-found]
        import soundfile as sf  # type: ignore[import-not-found]
        from opencc import OpenCC  # type: ignore[import-not-found]
        from tencentcloud.asr.v20190614.asr_client import models  # type: ignore[import-not-found]
        from batchalign._core.proto import AsrSegment, AsrWord

        cc = OpenCC("s2hk")

        # Prefer the ORIGINAL media file (byte-identical to BA2's upload); fall
        # back to writing the decoded PCM to a temp WAV.
        orig = str(item.source_id) if item.source_id is not None else ""
        local_path = orig
        tmp_path = ""
        if not (orig and pathlib.Path(orig).is_file()):
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
                tmp_path = tmp.name
            wave = np.frombuffer(item.audio.pcm_f32le, dtype=np.float32).copy()
            sf.write(tmp_path, wave, int(item.audio.sample_rate))
            local_path = tmp_path

        uid = str(uuid.uuid4())
        key = uid + pathlib.Path(local_path).suffix
        try:
            upload = self._cos.upload_file(
                Bucket=self._bucket, LocalFilePath=local_path, Key=key,
                PartSize=1, MAXThread=10, EnableMD5=False,
            )

            req = models.CreateRecTaskRequest()
            req.EngineModelType = (
                "16k_zh_large" if self._lang in _ZH_LARGE_LANGS else f"16k_{self._lang}"
            )
            req.ResTextFormat = 1
            req.SpeakerDiarization = 1
            req.ChannelNum = 1
            req.Url = upload["Location"]
            req.SourceType = 0
            resp = self._client.CreateRecTask(req)

            status_req = models.DescribeTaskStatusRequest()
            status_req.TaskId = resp.Data.TaskId
            res = self._client.DescribeTaskStatus(status_req)
            while res.Data.Status not in _DONE_STATUS:
                time.sleep(_POLL_INTERVAL_S)
                res = self._client.DescribeTaskStatus(status_req)
            if res.Data.Status in _FAILED_STATUS:
                raise RuntimeError(f"Tencent job failed: {res.Data.ErrorMsg}")

            self._cos.delete_object(Bucket=self._bucket, Key=upload["Key"])
        finally:
            if tmp_path:
                try:
                    pathlib.Path(tmp_path).unlink()
                except OSError:
                    pass

        segments_out: list[Any] = []
        for detail in res.Data.ResultDetail:
            start = detail.StartMs
            words: list[Any] = []
            for w in detail.Words:
                word = w.Word
                if self._lang == "yue":
                    word = self._replace_cantonese(cc.convert(word))
                words.append(
                    AsrWord(
                        text=word,
                        start_ms=int(w.OffsetStartMs + start),
                        end_ms=int(w.OffsetEndMs + start),
                        confidence=None,
                    )
                )
            if not words:
                continue
            joiner = "" if self._lang == "yue" else " "
            segments_out.append(
                AsrSegment(
                    start_ms=words[0].start_ms,
                    end_ms=words[-1].end_ms,
                    text=joiner.join(w.text for w in words).strip(),
                    speaker=str(detail.SpeakerId),
                    words=words,
                )
            )

        return AsrOutput(source_id=item.source_id, segments=segments_out)


__all__ = ["TencentAsrBackend"]
