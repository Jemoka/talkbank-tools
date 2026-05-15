# affects: batchalign/device.py
"""Tests for batchalign.device — DevicePolicy and resolve_inference_device.

Verifies the CPU/CUDA device resolution logic without loading any ML models.
MPS is never selected regardless of availability (AGXG14X kernel deadlock,
2026-04-05 incident).
"""

from __future__ import annotations

import pytest
import torch

from batchalign.device import DevicePolicy, resolve_inference_device


class TestResolveInferenceDevice:
    """Unit tests for the shared device resolution helper."""

    def test_returns_cpu_when_force_cpu(self, monkeypatch) -> None:
        """force_cpu=True must yield CPU regardless of hardware availability."""
        monkeypatch.setattr(torch.cuda, "is_available", lambda: True)
        device = resolve_inference_device(DevicePolicy(force_cpu=True))
        assert device == torch.device("cpu")

    def test_returns_cuda_when_available_and_not_force_cpu(
        self, monkeypatch
    ) -> None:
        """CUDA available + no force_cpu → select CUDA."""
        monkeypatch.setattr(torch.cuda, "is_available", lambda: True)
        device = resolve_inference_device(DevicePolicy(force_cpu=False))
        assert device == torch.device("cuda")

    def test_returns_cpu_when_no_cuda_and_not_force_cpu(
        self, monkeypatch
    ) -> None:
        """No CUDA available → fall through to CPU."""
        monkeypatch.setattr(torch.cuda, "is_available", lambda: False)
        device = resolve_inference_device(DevicePolicy(force_cpu=False))
        assert device == torch.device("cpu")

    def test_ignores_mps_even_when_available(self, monkeypatch) -> None:
        """MPS must never be selected — kernel deadlock has no user-space fix.

        Even if MPS is the only accelerator, the resolver must return CPU.
        Incident: 2026-04-05 AGXG14X GPU driver deadlock (two fleet machines).
        """
        monkeypatch.setattr(torch.cuda, "is_available", lambda: False)
        monkeypatch.setattr(torch.backends.mps, "is_available", lambda: True)
        device = resolve_inference_device(DevicePolicy(force_cpu=False))
        assert device == torch.device("cpu"), (
            "MPS must be excluded even when it is the only accelerator available"
        )

    def test_none_policy_defaults_to_no_force_cpu(self, monkeypatch) -> None:
        """Passing None uses DevicePolicy.from_environ(), which defaults force_cpu=False."""
        monkeypatch.setattr(torch.cuda, "is_available", lambda: True)
        # Ensure the env var is not set so from_environ() returns force_cpu=False
        monkeypatch.delenv("BATCHALIGN_FORCE_CPU", raising=False)
        device = resolve_inference_device(None)
        assert device == torch.device("cuda")

    def test_env_var_force_cpu_respected_via_none_policy(
        self, monkeypatch
    ) -> None:
        """BATCHALIGN_FORCE_CPU=1 via environment must force CPU even through None policy."""
        monkeypatch.setattr(torch.cuda, "is_available", lambda: True)
        monkeypatch.setenv("BATCHALIGN_FORCE_CPU", "1")
        device = resolve_inference_device(None)
        assert device == torch.device("cpu")
