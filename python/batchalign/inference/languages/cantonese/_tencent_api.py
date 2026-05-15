"""Tencent Cloud ASR client helpers for built-in HK/Cantonese engines."""

from __future__ import annotations

import json
import logging
import pathlib
import time
import uuid
import configparser
from typing import Any

_MAX_POLL_SECONDS = 600  # 10-minute safety timeout for ASR task polling

from batchalign.errors import ConfigError

from batchalign.inference._domain_types import AudioPath, LanguageCode, NumSpeakers

from ._asr_types import AsrGenerationPayload, TimedWord
from ._common import provider_lang_code, read_asr_config

L = logging.getLogger("batchalign.hk.tencent")

_CHINESE_CODES = {"zho", "yue", "wuu", "nan", "hak"}


class TencentRecognizer:
    """Thin wrapper around Tencent ASR transport plus Rust-owned projection."""

    def __init__(
        self,
        lang: LanguageCode,
        poll_interval_s: float = 10.0,
        *,
        config: configparser.ConfigParser | None = None,
    ) -> None:
        """Load credentials and initialize Tencent ASR/COS clients."""
        cfg = read_asr_config(
            (
                "engine.tencent.id",
                "engine.tencent.key",
                "engine.tencent.region",
                "engine.tencent.bucket",
            ),
            engine="Tencent",
            config=config,
        )

        try:
            from qcloud_cos import CosConfig, CosS3Client
            from tencentcloud.asr.v20190614.asr_client import AsrClient
            from tencentcloud.common.credential import Credential
        except Exception as exc:
            raise ImportError(
                "Tencent engine dependencies are missing from this "
                "environment. Reinstall batchalign3 or install the Tencent "
                "SDK packages."
            ) from exc

        secret_id = cfg["engine.tencent.id"]
        secret_key = cfg["engine.tencent.key"]
        region = cfg["engine.tencent.region"]

        self.lang_code = lang
        self.provider_lang = provider_lang_code(lang)
        self._poll_interval_s = max(1.0, poll_interval_s)
        self._bucket_name = cfg["engine.tencent.bucket"]
        self._region = region

        self._asr_client = AsrClient(Credential(secret_id, secret_key), region)
        self._bucket = CosS3Client(
            CosConfig(
                Region=region,
                SecretId=secret_id,
                SecretKey=secret_key,
                Token=None,
                Scheme="https",
            )
        )

    def _engine_model_type(self) -> str:
        """Return the Tencent engine model identifier for the configured language."""
        if self.lang_code in _CHINESE_CODES or self.provider_lang in _CHINESE_CODES:
            return "16k_zh_large"
        return f"16k_{self.provider_lang}"

    def transcribe(self, source_path: AudioPath, num_speakers: NumSpeakers = 0) -> list[Any]:
        """Upload media, submit ASR task, poll for completion, return ResultDetail."""
        try:
            from tencentcloud.asr.v20190614 import models
        except Exception as exc:
            raise ImportError(
                "Tencent engine dependencies are missing from this "
                "environment. Reinstall batchalign3 or install the Tencent "
                "SDK packages."
            ) from exc

        upload_key = f"{uuid.uuid4()}{pathlib.Path(source_path).suffix}"

        L.info("Tencent uploading '%s'", pathlib.Path(source_path).name)
        self._bucket.upload_file(
            Bucket=self._bucket_name,
            LocalFilePath=source_path,
            Key=upload_key,
            PartSize=1,
            MAXThread=10,
            EnableMD5=False,
        )

        media_url = (
            f"https://{self._bucket_name}.cos.{self._region}.myqcloud.com/{upload_key}"
        )

        create_req = models.CreateRecTaskRequest()
        create_req.EngineModelType = self._engine_model_type()
        create_req.ResTextFormat = 1
        create_req.SpeakerDiarization = 1
        if num_speakers > 0:
            create_req.SpeakerNumber = num_speakers
        create_req.ChannelNum = 1
        create_req.Url = media_url
        create_req.SourceType = 0

        try:
            create_resp = self._asr_client.CreateRecTask(create_req)
            task_id = int(create_resp.Data.TaskId)

            status_req = models.DescribeTaskStatusRequest()
            status_req.TaskId = task_id

            deadline = time.monotonic() + _MAX_POLL_SECONDS
            while True:
                status_resp = self._asr_client.DescribeTaskStatus(status_req)
                status = int(getattr(status_resp.Data, "Status", 0))
                if status in (2, 3):
                    if status == 3:
                        error_msg = str(getattr(status_resp.Data, "ErrorMsg", "unknown Tencent error"))
                        raise RuntimeError(f"Tencent ASR failed: {error_msg}")
                    result_detail = getattr(status_resp.Data, "ResultDetail", None)
                    return list(result_detail or [])
                if time.monotonic() > deadline:
                    raise RuntimeError(
                        f"Tencent ASR task {task_id} timed out after {_MAX_POLL_SECONDS}s"
                    )
                time.sleep(self._poll_interval_s)
        finally:
            try:
                self._bucket.delete_object(Bucket=self._bucket_name, Key=upload_key)
            except Exception:
                L.debug("Tencent cleanup failed for key=%s", upload_key, exc_info=True)

    def monologues(self, result_detail: list[Any]) -> AsrGenerationPayload:
        """Convert Tencent `ResultDetail` into shared ASR monologues via Rust."""
        return {"monologues": self._projection(result_detail)["monologues"]}

    def timed_words(self, result_detail: list[Any]) -> list[TimedWord]:
        """Extract timed words for `ParsedChat.add_utterance_timing()` via Rust."""
        return self._projection(result_detail)["timed_words"]

    def _projection(self, result_detail: list[Any]) -> dict[str, Any]:
        """Delegate Tencent result-detail projection to the shared Rust helper."""
        import batchalign_core

        return json.loads(
            batchalign_core.tencent_result_detail_to_asr(result_detail, self.lang_code)
        )
