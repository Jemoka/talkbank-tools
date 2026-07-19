"""Focused regressions for Stanza's same-version catalog refresh."""

from __future__ import annotations

import json
from types import SimpleNamespace

from batchalign.backends.morphosyntax.stanza import (
    _refresh_stanza_resources_manifest_if_present,
)


class _Response:
    def __init__(self, payload: bytes) -> None:
        self._payload = payload

    def __enter__(self) -> "_Response":
        return self

    def __exit__(self, *_args: object) -> None:
        return None

    def read(self) -> bytes:
        return self._payload


def _fake_stanza(model_dir, resources_url: str = "https://stanza.invalid"):
    common = SimpleNamespace(
        DEFAULT_MODEL_DIR=str(model_dir),
        DEFAULT_RESOURCES_URL=resources_url,
        DEFAULT_RESOURCES_VERSION="1.12.0",
    )
    return SimpleNamespace(resources=SimpleNamespace(common=common))


def test_refresh_replaces_stale_manifest_atomically(tmp_path):
    manifest = tmp_path / "resources.json"
    manifest.write_text(json.dumps({"marker": "stale"}))
    fresh = json.dumps({"marker": "fresh"}).encode()
    requested = {}

    def urlopen(url, *, timeout):
        requested.update(url=url, timeout=timeout)
        return _Response(fresh)

    _refresh_stanza_resources_manifest_if_present(
        _fake_stanza(tmp_path), urlopen=urlopen
    )

    assert json.loads(manifest.read_text()) == {"marker": "fresh"}
    assert requested == {
        "url": "https://stanza.invalid/resources_1.12.0.json",
        "timeout": 10,
    }
    assert not any(path.name.endswith(".tmp-refresh") for path in tmp_path.iterdir())


def test_refresh_failure_keeps_cached_manifest(tmp_path):
    manifest = tmp_path / "resources.json"
    stale = json.dumps({"marker": "stale"})
    manifest.write_text(stale)

    def offline(*_args, **_kwargs):
        raise OSError("offline")

    _refresh_stanza_resources_manifest_if_present(
        _fake_stanza(tmp_path), urlopen=offline
    )

    assert manifest.read_text() == stale
    assert not any(path.name.endswith(".tmp-refresh") for path in tmp_path.iterdir())


def test_missing_manifest_does_not_fetch(tmp_path):
    def unexpected_fetch(*_args, **_kwargs):
        raise AssertionError("fresh installs must use Stanza's bootstrap")

    _refresh_stanza_resources_manifest_if_present(
        _fake_stanza(tmp_path), urlopen=unexpected_fetch
    )
