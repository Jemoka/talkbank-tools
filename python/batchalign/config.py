"""Read batchalign user config from ``~/.batchalign.ini``.

This module is intentionally non-interactive. Setup is performed by the Rust
CLI command ``batchalign3 setup``; Python runtime code only reads config.
"""

from __future__ import annotations

import configparser
from dataclasses import dataclass
from pathlib import Path

from batchalign.errors import ConfigNotFoundError


@dataclass(frozen=True, slots=True)
class LegacyConfigRuntime:
    """Typed runtime inputs for locating the legacy ``.batchalign.ini`` file."""

    config_path: Path

    @classmethod
    def from_sources(
        cls,
        config_path: str | Path | None = None,
        home_dir: str | Path | None = None,
    ) -> LegacyConfigRuntime:
        """Build the config runtime from explicit path/home sources."""
        if config_path is not None and str(config_path).strip():
            return cls(config_path=Path(config_path).expanduser())

        if home_dir is not None and str(home_dir).strip():
            home_path = Path(home_dir).expanduser()
        else:
            home_path = Path.home()
        return cls(config_path=home_path / ".batchalign.ini")


def interactive_setup() -> configparser.ConfigParser:
    """Compatibility shim for removed Python interactive setup.

    Interactive setup was retired with the Python CLI/server migration.
    Use ``batchalign3 setup`` instead.
    """
    raise ConfigNotFoundError(
        "Interactive Python setup is retired. Run 'batchalign3 setup' to create "
        "or update ~/.batchalign.ini."
    )


def config_read(
    interactive: bool = False,
    *,
    runtime: LegacyConfigRuntime | None = None,
) -> configparser.ConfigParser:
    """Read ``~/.batchalign.ini`` and backfill required defaults.

    Parameters
    ----------
    interactive:
        Kept for backward compatibility. If ``True`` and the file is missing,
        this function raises with guidance to use ``batchalign3 setup``.
    """
    resolved_runtime = runtime or LegacyConfigRuntime.from_sources()
    config_path = resolved_runtime.config_path

    try:
        with open(config_path, "r+", encoding="utf-8") as df:
            config = configparser.ConfigParser()
            config.read_file(df)

            # Backfill legacy default expected by pipeline dispatch; keep file forward-compatible.
            if not config.has_option("ud", "model_version"):
                if not config.has_section("ud"):
                    config["ud"] = {}
                config["ud"]["model_version"] = "1.7.0"
                df.seek(0)
                config.write(df)
                df.truncate()

            return config
    except FileNotFoundError:
        if interactive:
            return interactive_setup()

        raise ConfigNotFoundError(
            f"Batchalign cannot find {config_path}. Run 'batchalign3 setup' to "
            "generate the legacy config file (for example when using Rev.AI keys)."
        )
