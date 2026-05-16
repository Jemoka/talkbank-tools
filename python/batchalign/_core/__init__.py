"""Re-exports from the compiled `batchalign._core` extension module.

Maturin builds the engine crate's cdylib and places it at
`batchalign/_core/_core.<abi>.so`. That .so registers symbols
(`Task`, `Pipeline`, `BAValue`, etc.) which we re-export from this
package so callers can write::

    from batchalign._core import Task, Pipeline

If the compiled extension is not present (fresh clone, missing
`maturin develop`), we raise a single readable ImportError pointing at
the build step rather than letting downstream attribute errors leak.
"""

from __future__ import annotations

# The compiled extension is exposed by maturin as `batchalign._core._core`
# (module-name = "batchalign._core" in pyproject.toml means the .so lands
# inside this package and exports its symbols at this import path).
#
# We try the symbol-import first; if the .so is missing, surface a clear
# diagnostic. Once the .so exists, every name listed in `_EXPECTED_NAMES`
# should be available.

_EXPECTED_NAMES = (
    "Task",
    "Pipeline",
    "BAValue",
    "MediaInput",
    "ChatInput",
    "PairedInput",
    "CacheSpec",
    "CachePolicy",
    "BatchPolicy",
    "ProgressEvent",
    "ProgressKind",
    "nuke_cache",
    "default_cache_path",
)

try:
    # maturin's `module-name = "batchalign._core"` means symbols are exposed
    # directly from this package on import.
    from batchalign._core._core import *  # type: ignore  # noqa: F401,F403
    from batchalign._core import _core as _ext  # type: ignore
    for _name in _EXPECTED_NAMES:
        if hasattr(_ext, _name):
            globals()[_name] = getattr(_ext, _name)
    from batchalign._core import proto  # noqa: F401
except ImportError as _exc:  # pragma: no cover
    raise ImportError(
        "The compiled batchalign._core extension is not available. "
        "Build it with `cd python && maturin develop` (or `maturin build`). "
        f"Underlying error: {_exc!r}"
    ) from _exc
