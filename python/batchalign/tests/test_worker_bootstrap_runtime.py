# affects: python/batchalign/worker/_main.py
# affects: python/batchalign/worker/_types.py
"""Tests for typed worker bootstrap runtime resolution."""

from __future__ import annotations

from batchalign.worker._main import build_worker_bootstrap_runtime, parse_worker_args
from batchalign.worker._types import InferTask


def test_worker_bootstrap_runtime_resolves_boundary_inputs() -> None:
    """CLI args and boundary env should become one typed bootstrap object."""

    args = parse_worker_args(
        [
            "--task",
            "asr",
            "--lang",
            "eng",
            "--num-speakers",
            "2",
            "--engine-overrides",
            '{"asr":"whisper"}',
            "--force-cpu",
        ]
    )

    bootstrap = build_worker_bootstrap_runtime(
        args,
        environ={
            "HOME": "/tmp/bootstrap-home",
            "BATCHALIGN_REV_API_KEY": " from-rust ",
        },
    )

    assert bootstrap.task is InferTask.ASR
    assert bootstrap.lang == "eng"
    assert bootstrap.num_speakers == 2
    assert bootstrap.engine_overrides == {"asr": "whisper"}
    assert bootstrap.device_policy.force_cpu is True
    assert bootstrap.revai_api_key == "from-rust"
