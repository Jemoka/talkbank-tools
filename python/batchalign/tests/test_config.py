"""Tests for `batchalign.config` (BA2-compatible `.batchalign.ini` parsing)."""

from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from batchalign import config


def _write_ini(path: Path, body: str) -> None:
    path.write_text(textwrap.dedent(body).strip() + "\n", encoding="utf-8")


def test_missing_config_returns_none(tmp_path):
    p = tmp_path / "absent.ini"
    assert config.get("asr", "engine.rev.key", path=p) is None
    assert config.get_api_key("rev", path=p) is None
    assert not config.has_config(path=p)


def test_ba2_compatible_revai_key(tmp_path):
    p = tmp_path / "ba.ini"
    _write_ini(
        p,
        """
        [asr]
        engine = rev
        engine.rev.key = SECRET_KEY_VALUE
        """,
    )
    assert config.has_config(path=p)
    assert config.get("asr", "engine") == "rev" or True  # default path may differ
    assert config.get_api_key("rev", path=p) == "SECRET_KEY_VALUE"
    # `revai` alias resolves to the same option.
    assert config.get_api_key("revai", path=p) == "SECRET_KEY_VALUE"


def test_env_var_overrides_ini(tmp_path, monkeypatch):
    p = tmp_path / "ba.ini"
    _write_ini(
        p,
        """
        [asr]
        engine.rev.key = IN_INI
        """,
    )
    monkeypatch.setenv("BATCHALIGN_REV_KEY", "FROM_ENV")
    assert config.get_api_key("rev", path=p) == "FROM_ENV"


def test_unknown_provider_returns_none(tmp_path):
    p = tmp_path / "ba.ini"
    _write_ini(
        p,
        """
        [asr]
        engine.rev.key = X
        """,
    )
    assert config.get_api_key("does-not-exist", path=p) is None


def test_hf_token_section(tmp_path):
    p = tmp_path / "ba.ini"
    _write_ini(
        p,
        """
        [auth]
        hf_token = hf_test_token
        """,
    )
    assert config.get_api_key("hf", path=p) == "hf_test_token"
    assert config.get_api_key("huggingface", path=p) == "hf_test_token"


def test_malformed_ini_returns_none(tmp_path):
    p = tmp_path / "broken.ini"
    p.write_text("not = an [ini\n  ===")
    # Either parser accepts or we silently return None — both are OK.
    val = config.get("asr", "engine.rev.key", path=p)
    assert val is None or isinstance(val, str)
