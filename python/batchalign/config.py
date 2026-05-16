"""Configuration loader for batchalign.

We read ``~/.batchalign.ini`` in **BA2-compatible** form so users
migrating from BA2 keep their credentials without manual editing.
The file uses ``configparser`` syntax with flat dotted keys::

    [asr]
    engine = rev
    engine.rev.key = <api key>

    engine.tencent.id     = <secret-id>
    engine.tencent.key    = <secret-key>
    engine.tencent.region = ap-guangzhou
    engine.tencent.bucket = <cos-bucket>

    engine.aliyun.ak_id     = <ak-id>
    engine.aliyun.ak_secret = <ak-secret>
    engine.aliyun.ak_appkey = <appkey>

    engine.openai.key = <api key>

    [translate]
    engine.google.key = <api key>

    [auth]
    hf_token = <huggingface token>

    [ud]
    model_version = 1.7.0

Two access shapes:

* :func:`get_api_key` for the common "one provider has one key" case
  (Rev.AI, OpenAI, HuggingFace, Google Translate).
* :func:`get_provider` for engines like Tencent / Aliyun that need
  several fields. Returns a dict of every ``engine.<provider>.<field>``
  pair found under the requested section.

All lookups also honor environment variables (``BATCHALIGN_<PROVIDER>_*``)
so CI and containerized runs can avoid touching the home directory.
"""

from __future__ import annotations

import configparser
import os
from pathlib import Path
from typing import Mapping

CONFIG_PATH = Path.home() / ".batchalign.ini"

# Map of provider names (used in `get_api_key`) to the (section, option)
# pair in BA2's ini layout. New providers go here.
_API_KEY_LOCATIONS: Mapping[str, tuple[str, str]] = {
    "rev": ("asr", "engine.rev.key"),
    "revai": ("asr", "engine.rev.key"),
    "openai": ("asr", "engine.openai.key"),
    "qwen": ("asr", "engine.aliyun.ak_secret"),
    "google_translate": ("translate", "engine.google.key"),
    "google": ("translate", "engine.google.key"),
    "hf": ("auth", "hf_token"),
    "huggingface": ("auth", "hf_token"),
}

# Map of provider -> (section, prefix). `get_provider("tencent")` returns
# every option under `[asr]` whose name starts with `engine.tencent.`, with
# the prefix stripped (e.g. ``engine.tencent.id`` → key ``id``).
_PROVIDER_LOCATIONS: Mapping[str, tuple[str, str]] = {
    "tencent": ("asr", "engine.tencent."),
    "aliyun": ("asr", "engine.aliyun."),
    "funasr": ("asr", "engine.funasr."),
    "qwen": ("asr", "engine.qwen."),
    "openai": ("asr", "engine.openai."),
    "rev": ("asr", "engine.rev."),
    "google": ("translate", "engine.google."),
}


def _load_config(path: Path = CONFIG_PATH) -> configparser.ConfigParser | None:
    """Read and return a parsed config, or ``None`` if the file doesn't exist."""
    if not path.exists():
        return None
    cfg = configparser.ConfigParser()
    try:
        cfg.read(path)
    except (configparser.Error, OSError):
        return None
    return cfg


def get(section: str, option: str, *, path: Path = CONFIG_PATH) -> str | None:
    """Generic config getter. Returns ``None`` if absent."""
    cfg = _load_config(path)
    if cfg is None or not cfg.has_option(section, option):
        return None
    return cfg.get(section, option)


def get_api_key(provider: str, *, path: Path = CONFIG_PATH) -> str | None:
    """Return the API key for ``provider``, or ``None`` if not configured.

    Resolution order:
      1. ``BATCHALIGN_<PROVIDER>_KEY`` env var (uppercased).
      2. ``~/.batchalign.ini`` per :data:`_API_KEY_LOCATIONS`.
    """
    env_var = f"BATCHALIGN_{provider.upper()}_KEY"
    env_val = os.environ.get(env_var)
    if env_val:
        return env_val
    loc = _API_KEY_LOCATIONS.get(provider.lower())
    if loc is None:
        return None
    return get(loc[0], loc[1], path=path)


def get_provider(
    provider: str,
    *,
    path: Path = CONFIG_PATH,
) -> dict[str, str]:
    """Return every ``engine.<provider>.<field>`` value as a flat dict.

    Empty dict when the section / prefix is absent. Environment variables
    of the form ``BATCHALIGN_<PROVIDER>_<FIELD>`` override the file values
    (uppercased; e.g. ``BATCHALIGN_TENCENT_ID``).
    """
    out: dict[str, str] = {}
    loc = _PROVIDER_LOCATIONS.get(provider.lower())
    if loc is not None:
        section, prefix = loc
        cfg = _load_config(path)
        if cfg is not None and cfg.has_section(section):
            for name, value in cfg.items(section):
                if name.startswith(prefix):
                    out[name[len(prefix):]] = value
    env_prefix = f"BATCHALIGN_{provider.upper()}_"
    for env_name, env_value in os.environ.items():
        if env_name.startswith(env_prefix):
            out[env_name[len(env_prefix):].lower()] = env_value
    return out


def has_config(path: Path = CONFIG_PATH) -> bool:
    """Whether ``~/.batchalign.ini`` exists and is readable."""
    return _load_config(path) is not None


__all__ = [
    "CONFIG_PATH",
    "get",
    "get_api_key",
    "get_provider",
    "has_config",
]
