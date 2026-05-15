"""Shared helpers for HK/Cantonese engines."""

from __future__ import annotations

import configparser
import logging
import os
from collections.abc import Mapping
from typing import Any

import pycountry

from batchalign.errors import ConfigError
from batchalign.config import config_read
from batchalign.inference._domain_types import LanguageCode

L = logging.getLogger("batchalign.hk")

# Type alias for engine overrides (replaces the old batchalign.providers.EngineOverrides import)
EngineOverrides = dict[str, str]

_ASR_ENV_KEYS: dict[str, str] = {
    "engine.tencent.id": "BATCHALIGN_TENCENT_ID",
    "engine.tencent.key": "BATCHALIGN_TENCENT_KEY",
    "engine.tencent.region": "BATCHALIGN_TENCENT_REGION",
    "engine.tencent.bucket": "BATCHALIGN_TENCENT_BUCKET",
    "engine.aliyun.ak_id": "BATCHALIGN_ALIYUN_AK_ID",
    "engine.aliyun.ak_secret": "BATCHALIGN_ALIYUN_AK_SECRET",
    "engine.aliyun.ak_appkey": "BATCHALIGN_ALIYUN_AK_APPKEY",
}


# ---------------------------------------------------------------------------
# Cantonese normalization — delegated to Rust (batchalign_core)
# ---------------------------------------------------------------------------

def normalize_cantonese_text(text: str) -> str:
    """Apply HK normalization: simplified→HK traditional + domain replacements.

    Delegates to ``batchalign_core.normalize_cantonese()`` (pure Rust, embedded
    OpenCC rules + Aho-Corasick replacement table).
    """
    import batchalign_core
    return batchalign_core.normalize_cantonese(text)


def normalize_cantonese_token(token: str, lang: LanguageCode) -> str:
    """Normalize a token if the language is Cantonese."""
    if lang != "yue":
        return token
    return normalize_cantonese_text(token)


def normalize_cantonese_char_tokens(text: str) -> list[str]:
    """Return per-character Cantonese tokens for timestamp alignment.

    Delegates to ``batchalign_core.cantonese_char_tokens()`` (pure Rust).
    """
    import batchalign_core
    return batchalign_core.cantonese_char_tokens(text)


def provider_lang_code(lang: LanguageCode) -> str:
    """Convert ISO-639-3 code to provider-specific code."""
    if lang == "yue":
        return "yue"
    try:
        match = pycountry.languages.get(alpha_3=lang)
        alpha2 = getattr(match, "alpha_2", None)
        if isinstance(alpha2, str) and alpha2:
            return alpha2
    except Exception:
        pass
    return lang


def read_asr_config(
    keys: tuple[str, ...],
    *,
    engine: str,
    config: configparser.ConfigParser | None = None,
    environ: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Read required ASR provider keys from injected env or configuration.

    Worker-launched HK providers should receive resolved credentials from the
    Rust control plane via environment variables. Direct Python callers may
    still fall back to an explicit or ambient `~/.batchalign.ini`.
    """
    env = environ if environ is not None else os.environ
    resolved_from_env: dict[str, str] = {}
    for key in keys:
        env_name = _ASR_ENV_KEYS.get(key)
        if env_name is None:
            break
        env_value = env.get(env_name, "").strip()
        if not env_value:
            resolved_from_env = {}
            break
        resolved_from_env[key] = env_value
    if len(resolved_from_env) == len(keys):
        return resolved_from_env

    resolved_config = config if config is not None else config_read()
    if not resolved_config.has_section("asr"):
        raise ConfigError(
            "No [asr] section in ~/.batchalign.ini. "
            f"{engine} requires provider credentials in that file."
        )

    missing = [k for k in keys if not resolved_config.has_option("asr", k)]
    if missing:
        raise ConfigError(
            f"Missing {engine} config keys in ~/.batchalign.ini: {', '.join(missing)}"
        )

    values: dict[str, str] = {}
    for k in keys:
        value = resolved_config.get("asr", k).strip()
        if not value:
            raise ConfigError(
                f"Empty {engine} config value in ~/.batchalign.ini: {k}"
            )
        values[k] = value
    return values


def parse_timestamp_pair(value: Any) -> tuple[int | None, int | None]:
    """Parse a FunASR timestamp item into milliseconds."""
    if not isinstance(value, (list, tuple)) or len(value) < 2:
        return None, None
    try:
        start = int(round(float(value[0])))
        end = int(round(float(value[1])))
    except Exception:
        return None, None
    return start, end
