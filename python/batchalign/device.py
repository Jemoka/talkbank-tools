"""Device selection helpers for CPU/GPU/MPS compute backends.

Controls whether batchalign engines use hardware accelerators (CUDA, MPS) or
fall back to CPU.  Runtime callers should prefer the typed
``DevicePolicy(force_cpu=...)`` boundary and keep any environment reads at the
process edge.

Typical usage from the CLI layer::

    if ctx.params["force_cpu"]:
        apply_force_cpu()
"""

from __future__ import annotations

from collections.abc import Mapping, MutableMapping
from dataclasses import dataclass
import os
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import torch as _torch


@dataclass(frozen=True, slots=True)
class DevicePolicy:
    """Typed device preference resolved once at the runtime boundary."""

    force_cpu: bool = False

    @classmethod
    def from_environ(cls, environ: Mapping[str, str] | None = None) -> DevicePolicy:
        """Build a policy from an environment mapping."""
        env = environ if environ is not None else os.environ
        return cls(force_cpu=env.get("BATCHALIGN_FORCE_CPU") == "1")


def apply_force_cpu(environ: MutableMapping[str, str] | None = None) -> DevicePolicy:
    """Set the ``BATCHALIGN_FORCE_CPU`` environment variable to ``"1"``.

    Call this early in the process (before any engine is instantiated) to
    force all subsequent engines onto CPU.  The flag is inherited by child
    processes spawned via ``ProcessPoolExecutor``.
    """
    env = environ if environ is not None else os.environ
    env["BATCHALIGN_FORCE_CPU"] = "1"
    return DevicePolicy(force_cpu=True)


def force_cpu_preferred(
    policy: DevicePolicy | None = None,
    *,
    environ: Mapping[str, str] | None = None,
) -> bool:
    """Check whether CPU-only mode has been requested.

    Returns
    -------
    bool
        ``True`` if the resolved device policy prefers CPU-only execution.
    """
    resolved_policy = policy or DevicePolicy.from_environ(environ)
    return resolved_policy.force_cpu


def resolve_inference_device(
    device_policy: DevicePolicy | None = None,
) -> _torch.device:
    """Resolve the concrete PyTorch device for ML model loading.

    The selection order is: CUDA > CPU. MPS is currently excluded, so model
    loading resolves to either CUDA or CPU only.

    Parameters
    ----------
    device_policy:
        Typed device preference. ``None`` reads ``BATCHALIGN_FORCE_CPU`` from
        the environment via ``DevicePolicy.from_environ()``.

    Returns
    -------
    torch.device
        Either ``torch.device("cuda")`` or ``torch.device("cpu")``.
    """
    import torch

    if force_cpu_preferred(device_policy):
        return torch.device("cpu")
    return torch.device("cuda") if torch.cuda.is_available() else torch.device("cpu")
