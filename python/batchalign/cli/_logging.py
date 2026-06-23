"""Verbosity-driven logging configuration for the CLI.

`configure(verbosity)` is called from the global Typer callback,
which runs BEFORE any subcommand body — and therefore before
backend modules import their heavy ML deps. That ordering is what
lets us silence `transformers`' import-time warnings.

The verbosity ladder:

    -q       batchalign=ERROR     libs=CRITICAL
    (none)   batchalign=WARNING   libs=ERROR
    -v       batchalign=INFO      libs=WARNING
    -vv      batchalign=DEBUG     libs=INFO
    -vvv     batchalign=DEBUG     libs=DEBUG

`-q` is encoded as `verbosity = -1`.

Library logger names are explicit — no wildcards. We only silence
loggers we know about.
"""

from __future__ import annotations

import logging
import os
import sys
import warnings

from rich.logging import RichHandler


# The third-party loggers we know are loud. Add to this list when a
# new noisy dep is introduced.
_NOISY_LIBRARIES = (
    "stanza",
    "torch",
    "transformers",
    "pyannote",
    "pyannote.audio",
    "speechbrain",
    "lightning_fabric",
    "onnxruntime",
    "urllib3",
    "huggingface_hub",
    "filelock",
    "librosa",
    "numba",
    "matplotlib",
)


def _level_for(verbosity: int, *, library: bool) -> int:
    """Map verbosity count to logging level."""
    if library:
        if verbosity <= -1:
            return logging.CRITICAL
        if verbosity == 0:
            return logging.ERROR
        if verbosity == 1:
            return logging.WARNING
        if verbosity == 2:
            return logging.INFO
        return logging.DEBUG
    # batchalign itself
    if verbosity <= -1:
        return logging.ERROR
    if verbosity == 0:
        return logging.WARNING
    if verbosity == 1:
        return logging.INFO
    return logging.DEBUG


def configure(verbosity: int) -> None:
    """Install a RichHandler on the root logger; set per-library levels.

    Idempotent — safe to call multiple times. Replaces any prior
    handlers we installed (identified by their `_batchalign_cli` mark).
    """
    if verbosity >= 2 and "BATCHALIGN_LOG" not in os.environ and "RUST_LOG" not in os.environ:
        os.environ["BATCHALIGN_LOG"] = "batchalign=debug"

    root = logging.getLogger()
    # Remove any prior handlers we installed; leave foreign handlers
    # (e.g. pytest's capture) alone.
    for h in list(root.handlers):
        if getattr(h, "_batchalign_cli", False):
            root.removeHandler(h)

    handler = RichHandler(
        rich_tracebacks=(verbosity >= 2),
        show_path=False,
        show_time=False,
        markup=False,
    )
    handler._batchalign_cli = True  # type: ignore[attr-defined]
    handler.setLevel(_level_for(verbosity, library=True))
    root.addHandler(handler)
    # Root must be at least as permissive as the most verbose channel.
    root.setLevel(min(
        _level_for(verbosity, library=True),
        _level_for(verbosity, library=False),
    ))

    logging.getLogger("batchalign").setLevel(_level_for(verbosity, library=False))
    for name in _NOISY_LIBRARIES:
        logging.getLogger(name).setLevel(_level_for(verbosity, library=True))

    # Library-specific knobs that bypass standard logging.
    _silence_transformers(verbosity)
    _silence_warnings(verbosity)


def _silence_transformers(verbosity: int) -> None:
    """Transformers has its own logging gate alongside stdlib logging.

    Set the `TRANSFORMERS_VERBOSITY` env var rather than importing
    `transformers` here — importing it costs ~3s, and lightweight
    commands (`cache --help`, `version`, top-level `--help`) never
    need it loaded. Transformers reads this env var during its own
    import, so the silencing still applies whenever a backend later
    pulls it in. If it's already been imported (e.g. inside a long-
    running daemon process), apply the runtime knob too.
    """
    if verbosity >= 2:
        return
    os.environ.setdefault("TRANSFORMERS_VERBOSITY", "error")
    mod = sys.modules.get("transformers")
    if mod is not None:
        try:
            mod.logging.set_verbosity_error()
        except Exception:  # noqa: BLE001
            pass


def _silence_warnings(verbosity: int) -> None:
    """Python warnings: hide everything below -v."""
    if verbosity >= 1:
        return
    # FutureWarning from torch/transformers is the worst offender.
    warnings.filterwarnings("ignore", category=FutureWarning)
    warnings.filterwarnings("ignore", category=UserWarning)
    warnings.filterwarnings("ignore", category=DeprecationWarning)
    os.environ.setdefault("PYTHONWARNINGS", "ignore")


__all__ = ["configure"]
